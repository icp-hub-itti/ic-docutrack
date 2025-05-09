mod canister;
mod client;
mod storage;
mod utils;

use candid::Principal;
use did::orchestrator::{
    GetUsersResponse, OrchestratorInitArgs, PublicKey, RetryUserCanisterCreationResponse,
    SetUserResponse, UserCanisterResponse, WhoamiResponse,
};
use ic_cdk_macros::{init, query, update};

use self::canister::Canister;
use self::storage::config::Config;

#[init]
pub fn init(args: OrchestratorInitArgs) {
    Canister::init(args);
}

#[query]
pub fn get_users() -> GetUsersResponse {
    Canister::get_users()
}

#[query]
pub fn orbit_station() -> Principal {
    Config::get_orbit_station()
}

#[update]
pub fn retry_user_canister_creation() -> RetryUserCanisterCreationResponse {
    Canister::retry_user_canister_creation()
}

#[update]
pub fn set_user(username: String, public_key: PublicKey) -> SetUserResponse {
    Canister::set_user(username, public_key)
}

#[query]
pub fn username_exists(username: String) -> bool {
    Canister::username_exists(username)
}

#[query]
pub fn user_canister() -> UserCanisterResponse {
    Canister::user_canister()
}

#[query]
pub fn who_am_i() -> WhoamiResponse {
    Canister::whoami()
}

ic_cdk::export_candid!();
