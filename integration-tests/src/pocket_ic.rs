mod cycles;

use std::io::Read as _;
use std::path::PathBuf;

use candid::{CandidType, Decode, Encode, Principal};
use did::orbit_station::{
    AdminInitInput, HealthStatus, ListUsersInput, ListUsersResult, SystemInit, SystemInstall,
    SystemUpgraderInput,
};
use pocket_ic::nonblocking::PocketIc;
use serde::de::DeserializeOwned;

use crate::TestEnv;
use crate::actor::admin;
use crate::wasm::Canister;

const DEFAULT_CYCLES: u128 = 2_000_000_000_000_000;
const NNS_ROOT_CANISTER_ID: Principal = Principal::from_slice(&[0, 0, 0, 0, 0, 0, 0, 3, 1, 1]);

/// Test environment
pub struct PocketIcTestEnv {
    pub pic: PocketIc,
    pub orchestrator: Principal,
    pub user_canister1: Principal,
    
    pub orbit_station: Principal,
    /// Uuid of the station admin
    station_admin: String,
}

impl TestEnv for PocketIcTestEnv {
    fn orchestrator(&self) -> Principal {
        self.orchestrator
    }
    fn user_canister1(&self) -> Principal {
        self.user_canister1
    }

    fn orbit_station(&self) -> Principal {
        self.orbit_station
    }

    fn station_admin(&self) -> String {
        self.station_admin.clone()
    }
    fn admin(&self) -> Principal {
        admin()
    }

    async fn query<R>(
        &self,
        canister: Principal,
        caller: Principal,
        method: &str,
        payload: Vec<u8>,
    ) -> anyhow::Result<R>
    where
        R: DeserializeOwned + CandidType,
    {
        let reply = match self.pic.query_call(canister, caller, method, payload).await {
            Ok(result) => result,
            Err(e) => anyhow::bail!("Error calling {}: {:?}", method, e),
        };
        let ret_type = Decode!(&reply, R)?;

        Ok(ret_type)
    }

    async fn update<R>(
        &self,
        canister: Principal,
        caller: Principal,
        method: &str,
        payload: Vec<u8>,
    ) -> anyhow::Result<R>
    where
        R: DeserializeOwned + CandidType,
    {
        let reply = if self.is_live() {
            let id = self
                .pic
                .submit_call(canister, caller, method, payload)
                .await
                .map_err(|e| anyhow::anyhow!("Error submitting call {}: {:?}", method, e))?;
            self.pic.await_call_no_ticks(id).await
        } else {
            self.pic
                .update_call(canister, caller, method, payload)
                .await
        };

        let reply = match reply {
            Ok(r) => r,
            Err(r) => anyhow::bail!("{} was rejected: {:?}", method, r),
        };
        let ret_type = Decode!(&reply, R)?;

        Ok(ret_type)
    }
}

impl PocketIcTestEnv {
    /// Install the canisters needed for the tests
    pub async fn init() -> Self {
        println!("Initializing Pocket IC test environment...");
        let pic = ic_exports::pocket_ic::init_pocket_ic()
            .await
            .with_nns_subnet()
            .with_ii_subnet()
            .with_fiduciary_subnet()
            .with_application_subnet()
            .with_max_request_time_ms(Some(30_000))
            .build_async()
            .await;

        // create canisters
        let orchestrator = pic.create_canister_with_settings(Some(admin()), None).await;
        println!("Orchestrator: {orchestrator}",);
        let user_canister1 = pic.create_canister().await;
        println!("User canister: {user_canister1}",);
        let orbit_station = pic.create_canister_with_settings(Some(admin()), None).await;
        println!("Orbit station: {orbit_station}",);

        // set controllers for station
        pic.set_controllers(orbit_station, Some(admin()), vec![admin(), orbit_station])
            .await
            .expect("Failed to set controllers");

        // setup cmc
        cycles::setup_cycles_minting_canister(&pic).await;

        // install orbit station
        Self::install_orbit_station(&pic, orbit_station).await;
        // install the wasm canister
        Self::install_backend(&pic, orchestrator, "orchestrator").await;
        Self::install_backend(&pic, user_canister1, "usercan").await;

        // get station admin
        let station_admin = Self::get_station_admin(&pic, orbit_station).await;
        println!("Station admin: {station_admin}",);

        Self {
            orchestrator, user_canister1,
            pic,
            orbit_station,
            station_admin,
        }
    }

    fn is_live(&self) -> bool {
        self.pic.url().is_some()
    }

    async fn install_backend(pic: &PocketIc, canister_id: Principal, canister_type: &str) {
        pic.add_cycles(canister_id, DEFAULT_CYCLES).await;

        // let wasm_bytes = Self::load_wasm(Canister::Backend);
        let wasm_bytes = Self::load_wasm(match canister_type {
            "orchestrator" => Canister::Orchestrator,
            "usercan" => Canister::UserCan,
            _ => panic!("Unknown canister type"),
        });
        println!("Wasm size: {} for {}", wasm_bytes.len(), canister_type);

        //let init_arg = todo!();
        let init_arg = vec![]; // Encode!(&init_arg).unwrap();

        pic.install_canister(canister_id, wasm_bytes, init_arg, Some(admin()))
            .await;
    }

    /// Install [`Canister::OrbitStation`] canister
    async fn install_orbit_station(pic: &PocketIc, canister_id: Principal) {
        pic.add_cycles(canister_id, DEFAULT_CYCLES).await;
        let wasm_bytes = Self::load_wasm(Canister::OrbitStation);

        let init_arg = Some(SystemInstall::Init(SystemInit {
            name: "Station".to_string(),
            assets: None,
            fallback_controller: Some(NNS_ROOT_CANISTER_ID),
            upgrader: SystemUpgraderInput::Deploy {
                initial_cycles: Some(5_000_000_000_000u64.into()),
                wasm_module: Self::load_wasm(Canister::OrbitUpgrader).into(),
            },
            accounts: None,
            admins: vec![AdminInitInput {
                name: "station-admin".to_string(),
                identity: admin(),
            }],
            quorum: Some(1),
        }));

        let init_arg = Encode!(&init_arg).expect("Failed to encode init arg");
        pic.install_canister(canister_id, wasm_bytes, init_arg, Some(admin()))
            .await;

        // wait for the station to be healthy
        Self::await_station_healthy(pic, canister_id).await;
    }

    /// Wait for the station to be healthy
    async fn await_station_healthy(pic: &PocketIc, station_id: Principal) {
        let max_rounds = 100;
        for _ in 0..max_rounds {
            pic.tick().await;

            let payload = Encode!(&()).expect("Failed to encode payload");
            let reply = pic
                .query_call(station_id, admin(), "health_status", payload)
                .await
                .expect("Unexpected error calling Station health_status");
            let ret_type: HealthStatus =
                Decode!(&reply, HealthStatus).expect("Failed to decode health status");

            if matches!(ret_type, HealthStatus::Healthy) {
                return;
            }
        }
        panic!(
            "Station did not become healthy within {} rounds.",
            max_rounds
        );
    }

    /// Get the station admin
    async fn get_station_admin(pic: &PocketIc, station_id: Principal) -> String {
        let payload = ListUsersInput {
            groups: None,
            statuses: None,
            paginate: None,
            search_term: None,
        };
        let payload = Encode!(&(payload,)).expect("Failed to encode payload");
        let reply = pic
            .query_call(station_id, admin(), "list_users", payload)
            .await
            .expect("Unexpected error calling Station station_admin");
        let ret_type: ListUsersResult =
            Decode!(&reply, ListUsersResult).expect("Failed to decode station admin");

        let res = ret_type.expect("Failed to get users");
        if res.users.is_empty() {
            panic!("No users found");
        }

        let admin = res
            .users
            .into_iter()
            .find(|u| u.name == "station-admin")
            .expect("Failed to find station admin");

        admin.id
    }

    fn load_wasm(canister: Canister) -> Vec<u8> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push(canister.as_path());

        let mut file = std::fs::File::open(path).unwrap();
        let mut wasm_bytes = Vec::new();
        file.read_to_end(&mut wasm_bytes).unwrap();

        wasm_bytes
    }

    pub async fn live(&mut self, live: bool) {
        if live {
            self.pic.make_live(None).await;
        } else {
            self.pic.stop_live().await;
        }
    }
}
