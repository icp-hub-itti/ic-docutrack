use std::time::Duration;

use candid::Principal;
use did::orchestrator::{
    GetUsersResponse, PublicKey, SetUserResponse, UserCanisterResponse, WhoamiResponse,
};

use super::PocketIcTestEnv;
use crate::TestEnv as _;
use crate::actor::admin;

pub struct OrchestratorClient<'a> {
    pic: &'a PocketIcTestEnv,
}

impl<'a> From<&'a PocketIcTestEnv> for OrchestratorClient<'a> {
    fn from(pic: &'a PocketIcTestEnv) -> Self {
        Self { pic }
    }
}

impl OrchestratorClient<'_> {
    pub async fn orchestrator_client(&self) -> Principal {
        self.pic
            .query::<Principal>(self.pic.orchestrator(), admin(), "orbit_station", vec![])
            .await
            .expect("Failed to get orbit station")
    }

    pub async fn get_users(&self, caller: Principal) -> GetUsersResponse {
        let payload = candid::encode_args(()).unwrap();
        self.pic
            .query::<GetUsersResponse>(self.pic.orchestrator(), caller, "get_users", payload)
            .await
            .expect("Failed to get users")
    }

    pub async fn set_user(
        &self,
        caller: Principal,
        username: String,
        public_key: PublicKey,
    ) -> SetUserResponse {
        let payload = candid::encode_args((username, public_key)).unwrap();
        self.pic
            .update::<SetUserResponse>(self.pic.orchestrator(), caller, "set_user", payload)
            .await
            .expect("Failed to set user")
    }

    pub async fn who_am_i(&self, caller: Principal) -> WhoamiResponse {
        let payload = candid::encode_args(()).unwrap();
        self.pic
            .query::<WhoamiResponse>(self.pic.orchestrator(), caller, "who_am_i", payload)
            .await
            .expect("Failed to get who am i")
    }

    pub async fn user_canister(&self, caller: Principal) -> UserCanisterResponse {
        let payload = candid::encode_args(()).unwrap();
        self.pic
            .query::<UserCanisterResponse>(
                self.pic.orchestrator(),
                caller,
                "user_canister",
                payload,
            )
            .await
            .expect("Failed to get user canister")
    }

    pub async fn username_exists(&self, username: String) -> bool {
        let payload = candid::encode_args((username,)).unwrap();
        self.pic
            .query::<bool>(self.pic.orchestrator(), admin(), "username_exists", payload)
            .await
            .expect("Failed to check if username exists")
    }

    /// Wait for the user canister to be created
    ///
    /// This function will keep querying the user canister until it is created or fails.
    ///
    /// Returns the user canister ID if it is created successfully.
    ///
    /// ## Panics
    ///
    /// - If the user canister creation fails
    /// - If the caller is anonymous
    /// - If the user canister is uninitialized
    pub async fn wait_for_user_canister(&self, caller: Principal) -> Principal {
        loop {
            let state = self.user_canister(caller).await;
            match state {
                UserCanisterResponse::Ok(canister_id) => return canister_id,
                UserCanisterResponse::CreationFailed { reason } => {
                    panic!("User canister creation failed: {}", reason);
                }
                UserCanisterResponse::AnonymousCaller => {
                    panic!("Anonymous caller cannot create user canister");
                }
                UserCanisterResponse::CreationPending => {
                    self.pic.pic.advance_time(Duration::from_secs(5)).await;
                    self.pic.pic.tick().await;
                }
                UserCanisterResponse::Uninitialized => {
                    panic!("User canister is uninitialized");
                }
            }
        }
    }
}
