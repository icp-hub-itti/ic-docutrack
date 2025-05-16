mod shared_files;
mod user;
mod user_canister;
mod whoami;

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

pub use self::shared_files::{
    FileId, RevokeShareFileResponse, ShareFileResponse, SharedFilesResponse,
};
pub use self::user::{
    GetUsersResponse, MAX_USERNAME_SIZE, PUBKEY_SIZE, PublicKey, PublicUser, SetUserResponse, User,
};
pub use self::user_canister::{RetryUserCanisterCreationResponse, UserCanisterResponse};
pub use self::whoami::WhoamiResponse;

/// Orchestrator canister install arguments
#[derive(Debug, CandidType, Serialize, Deserialize)]
pub enum OrchestratorInstallArgs {
    /// Arguments for the `init` method
    Init(OrchestratorInitArgs),
    /// Arguments for the `post_upgrade` method
    Upgrade,
}

/// Orchestrator canister `init` arguments
#[derive(Debug, CandidType, Serialize, Deserialize)]
pub struct OrchestratorInitArgs {
    /// UUID of the Orbit Station admin
    pub orbit_station_admin: String,
    /// Principal of the Orbit Station canister
    pub orbit_station: Principal,
}
