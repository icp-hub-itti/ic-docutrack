mod file;

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

pub use self::file::{
    AliasInfo, ENCRYPTION_KEY_SIZE, FileData, FileDownloadResponse, FileSharingResponse,
    FileStatus, GetAliasInfoError, OwnerKey, PublicFileMetadata, UploadFileAtomicRequest,
    UploadFileContinueRequest, UploadFileError, UploadFileRequest,
};

/// User Canister canister install arguments.
#[derive(Debug, CandidType, Serialize, Deserialize)]
pub enum UserCanisterInstallArgs {
    /// Arguments for the `init` method
    Init(UserCanisterInitArgs),
    /// Arguments for the `post_upgrade` method
    Upgrade,
}

/// User Canister canister init arguments.
#[derive(Debug, CandidType, Serialize, Deserialize)]
pub struct UserCanisterInitArgs {
    pub orchestrator: Principal,
    pub owner: Principal,
}
