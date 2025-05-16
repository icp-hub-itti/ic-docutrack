mod aliases;
mod canister;
mod client;
pub mod inspect;
mod storage;
mod utils;

use candid::Principal;
use did::orchestrator::PublicKey;
use did::user_canister::{
    AliasInfo, FileDownloadResponse, FileSharingResponse, GetAliasInfoError, OwnerKey,
    PublicFileMetadata, UploadFileAtomicRequest, UploadFileContinueRequest, UploadFileError,
    UploadFileRequest, UserCanisterInstallArgs,
};
use ic_cdk_macros::{init, query, update};
use storage::config::Config;
use utils::msg_caller;

use self::canister::Canister;

#[init]
pub fn init(args: UserCanisterInstallArgs) {
    Canister::init(args);
}

#[query]
fn public_key() -> PublicKey {
    Config::get_owner_public_key()
}

#[update]
fn set_public_key(public_key: PublicKey) {
    Config::set_owner_public_key(msg_caller(), public_key);
}

#[query]
fn get_requests() -> Vec<PublicFileMetadata> {
    Canister::get_requests(msg_caller())
}

#[query]
fn get_shared_files(user_id: Principal) -> Vec<PublicFileMetadata> {
    Canister::get_shared_files(msg_caller(), user_id)
}

#[query]
fn get_alias_info(alias: String) -> Result<AliasInfo, GetAliasInfoError> {
    Canister::get_alias_info(alias)
}

#[update]
fn upload_file(request: UploadFileRequest) -> Result<(), UploadFileError> {
    Canister::upload_file(
        request.file_id,
        request.file_content,
        request.file_type,
        request.owner_key,
        request.num_chunks,
    )
}

#[update]
fn upload_file_atomic(request: UploadFileAtomicRequest) -> u64 {
    Canister::upload_file_atomic(msg_caller(), request)
}

#[update]
fn upload_file_continue(request: UploadFileContinueRequest) {
    Canister::upload_file_continue(request)
}

#[update]
async fn request_file(request_name: String) -> String {
    Canister::request_file(msg_caller(), request_name).await
}

#[query]
fn download_file(file_id: u64, chunk_id: u64) -> FileDownloadResponse {
    Canister::download_file(msg_caller(), file_id, chunk_id)
}

#[update]
async fn share_file(
    user_id: Principal,
    file_id: u64,
    file_key_encrypted_for_user: OwnerKey,
) -> FileSharingResponse {
    Canister::share_file(msg_caller(), user_id, file_id, file_key_encrypted_for_user).await
}

#[update]
async fn share_file_with_users(
    user_id: Vec<Principal>,
    file_id: u64,
    file_key_encrypted_for_user: Vec<OwnerKey>,
) {
    Canister::share_file_with_users(msg_caller(), user_id, file_id, file_key_encrypted_for_user)
        .await
}

#[update]
async fn revoke_share(user_id: Principal, file_id: u64) {
    Canister::revoke_file_sharing(msg_caller(), user_id, file_id).await
}

ic_cdk::export_candid!();
