use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

/// Response for `user_canister` query
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum UserCanisterResponse {
    /// The user canister is created and ready to use
    Ok(Principal),
    /// The user canister is being created
    CreationPending,
    /// The user canister creation failed; returns the reason
    CreationFailed { reason: String },
    /// The creation is not started yet
    Uninitialized,
    /// Called with an anonymous caller
    AnonymousCaller,
}

/// Response for `retry_user_canister_creation` query
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RetryUserCanisterCreationResponse {
    /// The user canister is being retried
    Ok,
    /// The user canister exists.
    Created(Principal),
    /// Creation is already in progress
    CreationPending,
    /// Anonymous caller
    AnonymousCaller,
    /// User not found - use `set_user` first to create a user
    UserNotFound,
}
