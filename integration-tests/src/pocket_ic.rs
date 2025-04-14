use std::io::Read as _;
use std::path::PathBuf;

use candid::{CandidType, Decode, Encode, Principal};
use did::orbit_station::{AdminInitInput, SystemInit, SystemInstall, SystemUpgraderInput};
use pocket_ic::nonblocking::PocketIc;
use serde::de::DeserializeOwned;

use crate::TestEnv;
use crate::wasm::Canister;

const DEFAULT_CYCLES: u128 = 2_000_000_000_000_000;

/// Test environment
pub struct PocketIcTestEnv {
    pub pic: PocketIc,
    pub backend: Principal,
    pub orbit_station: Principal,
}

impl TestEnv for PocketIcTestEnv {
    fn backend(&self) -> Principal {
        self.backend
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
        let pic = ic_exports::pocket_ic::init_pocket_ic()
            .await
            .with_nns_subnet()
            .with_ii_subnet() // To have ECDSA keys
            .with_application_subnet()
            .with_max_request_time_ms(Some(30_000))
            .build_async()
            .await;

        // create canisters
        let backend = pic.create_canister().await;
        println!("Backend: {backend}",);
        let orbit_station = pic.create_canister().await;
        println!("Orbit station: {orbit_station}",);

        // install orbit station
        Self::install_orbit_station(&pic, orbit_station, backend).await;
        // install the backend canister
        Self::install_backend(&pic, backend).await;

        Self {
            backend,
            pic,
            orbit_station,
        }
    }

    fn is_live(&self) -> bool {
        self.pic.url().is_some()
    }

    /// Install [`Canister::Backend`] canister
    async fn install_backend(pic: &PocketIc, canister_id: Principal) {
        pic.add_cycles(canister_id, DEFAULT_CYCLES).await;

        let wasm_bytes = Self::load_wasm(Canister::Backend);

        //let init_arg = todo!();
        let init_arg = vec![]; // Encode!(&init_arg).unwrap();

        pic.install_canister(canister_id, wasm_bytes, init_arg, None)
            .await;
    }

    /// Install [`Canister::OrbitStation`] canister
    async fn install_orbit_station(pic: &PocketIc, canister_id: Principal, backend: Principal) {
        pic.add_cycles(canister_id, DEFAULT_CYCLES).await;
        let wasm_bytes = Self::load_wasm(Canister::OrbitStation);

        let init_arg = Some(SystemInstall::Init(SystemInit {
            name: "orbit_station".to_string(),
            assets: None,
            fallback_controller: None,
            upgrader: SystemUpgraderInput::Id(canister_id),
            accounts: None,
            admins: vec![AdminInitInput {
                name: "backend".to_string(),
                identity: backend,
            }],
            quorum: None,
        }));

        let init_arg = Encode!(&init_arg).expect("Failed to encode init arg");
        pic.install_canister(canister_id, wasm_bytes, init_arg, None)
            .await;
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
