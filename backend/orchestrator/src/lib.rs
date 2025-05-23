mod canister;
mod client;
mod storage;
mod utils;

use candid::Principal;
use did::orchestrator::{
    FileId, GetUsersResponse, OrchestratorInstallArgs, Pagination, PublicKey,
    RetryUserCanisterCreationResponse, RevokeShareFileResponse, SetUserResponse, ShareFileMetadata,
    ShareFileResponse, SharedFilesResponse, UserCanisterResponse, WhoamiResponse,
};
use ic_cdk_macros::{init, query, update};

use self::canister::Canister;
use self::storage::config::Config;

#[init]
pub fn init(args: OrchestratorInstallArgs) {
    Canister::init(args);
}

#[query]
pub fn get_users(pagination: Pagination) -> GetUsersResponse {
    Canister::get_users(pagination)
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
pub fn revoke_share_file(user: Principal, file_id: FileId) -> RevokeShareFileResponse {
    Canister::revoke_share_file(user, file_id)
}

#[update]
pub fn revoke_share_file_for_users(
    users: Vec<Principal>,
    file_id: FileId,
) -> RevokeShareFileResponse {
    Canister::revoke_share_file_for_users(users, file_id)
}

#[update]
pub fn set_user(username: String, public_key: PublicKey) -> SetUserResponse {
    Canister::set_user(username, public_key)
}

#[update]
pub fn share_file(
    user: Principal,
    file_id: FileId,
    metadata: ShareFileMetadata,
) -> ShareFileResponse {
    Canister::share_file(user, file_id, metadata)
}

#[update]
pub fn share_file_with_users(
    users: Vec<Principal>,
    file_id: FileId,
    metadata: ShareFileMetadata,
) -> ShareFileResponse {
    Canister::share_file_with_users(users, file_id, metadata)
}

#[query]
pub fn shared_files() -> SharedFilesResponse {
    Canister::shared_files()
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
