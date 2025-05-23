mod delete_file;
mod file;
mod owner_key;

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

pub use self::delete_file::DeleteFileResponse;
pub use self::file::{
    AliasInfo, FileData, FileDownloadResponse, FileSharingResponse, FileStatus, GetAliasInfoError,
    PublicFileMetadata, UploadFileAtomicRequest, UploadFileContinueRequest,
    UploadFileContinueResponse, UploadFileError, UploadFileRequest,
};
pub use self::owner_key::OwnerKey;
pub use crate::public_key::PublicKey;

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
