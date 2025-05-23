use candid::Principal;
use did::orchestrator::{FileId, RevokeShareFileResponse, ShareFileMetadata, ShareFileResponse};
use ic_cdk::call::{Call, CallResult, Error as CallError};

/// Orchestrator canister client.
pub struct OrchestratorClient {
    principal: Principal,
}

impl From<Principal> for OrchestratorClient {
    fn from(principal: Principal) -> Self {
        Self::new(principal)
    }
}

impl OrchestratorClient {
    pub fn new(principal: Principal) -> Self {
        Self { principal }
    }

    /// Revoke share file from user.
    ///
    /// If successful, returns [`RevokeShareFileResponse`], which means that the call was successful, but it's not
    /// guaranteed that the operation was successful and so it should be checked.
    pub async fn revoke_share_file(
        &self,
        user: Principal,
        file_id: FileId,
    ) -> CallResult<RevokeShareFileResponse> {
        Call::unbounded_wait(self.principal, "revoke_share_file")
            .with_args(&(user, file_id))
            .await
            .map_err(CallError::from)?
            .candid::<RevokeShareFileResponse>()
            .map_err(CallError::CandidDecodeFailed)
    }

    /// Revoke share file for multiple users.
    ///
    /// If successful, returns [`RevokeShareFileResponse`], which means that the call was successful, but it's not
    /// guaranteed that the operation was successful and so it should be checked.
    pub async fn revoke_share_file_for_users(
        &self,
        users: &[Principal],
        file_id: FileId,
    ) -> CallResult<RevokeShareFileResponse> {
        Call::unbounded_wait(self.principal, "revoke_share_file_for_users")
            .with_args(&(users, file_id))
            .await
            .map_err(CallError::from)?
            .candid::<RevokeShareFileResponse>()
            .map_err(CallError::CandidDecodeFailed)
    }

    /// Share file with user.
    pub async fn share_file(
        &self,
        user: Principal,
        file_id: FileId,
        metadata: ShareFileMetadata,
    ) -> CallResult<ShareFileResponse> {
        Call::unbounded_wait(self.principal, "share_file")
            .with_args(&(user, file_id, metadata))
            .await
            .map_err(CallError::from)?
            .candid::<ShareFileResponse>()
            .map_err(CallError::CandidDecodeFailed)
    }

    /// Share file with multiple users.
    pub async fn share_file_with_users(
        &self,
        users: &[Principal],
        file_id: FileId,
        metadata: ShareFileMetadata,
    ) -> CallResult<ShareFileResponse> {
        Call::unbounded_wait(self.principal, "share_file_with_users")
            .with_args(&(users, file_id, metadata))
            .await
            .map_err(CallError::from)?
            .candid::<ShareFileResponse>()
            .map_err(CallError::CandidDecodeFailed)
    }
}
