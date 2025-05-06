// mod aliases;
pub mod api;
mod memory;
mod upgrade;

use std::cell::RefCell;
use std::collections::BTreeMap;
// use std::ops::Bound::{Excluded, Included};

use candid::{CandidType, Principal};
use ic_cdk::api::caller;
use ic_cdk_macros::{post_upgrade, pre_upgrade, query, update};
// use ic_stable_structures::StableBTreeMap;
// use memory::Memory;
use serde::{Deserialize, Serialize};

// use crate::aliases::{AliasGenerator, Randomness};
// use crate::api::UploadFileAtomicRequest;

thread_local! {
    /// Initialize the state randomness with the current time.
    static STATE: RefCell<State> = RefCell::new(State::new(&get_randomness_seed()[..]));
}

// type FileId = u64;
// type ChunkId = u64;

#[derive(CandidType, Serialize, Deserialize, Clone,Debug,/* PartialEq, Eq*/)]
pub struct User {
    pub username: String,
    pub public_key: Vec<u8>,
    pub canister_id: Principal,
}


#[derive(CandidType, Serialize, Deserialize)]
pub enum SetUserResponse {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "username_exists")]
    UsernameExists,
}

#[derive(CandidType, Serialize, Deserialize)]
pub enum WhoamiResponse {
    #[serde(rename = "known_user")]
    KnownUser(PublicUser),
    #[serde(rename = "unknown_user")]
    UnknownUser,
}


#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GetAliasInfoError {
    #[serde(rename = "not_found")]
    NotFound,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AliasInfo {
    pub file_id: u64,
    pub file_name: String,
    pub user: PublicUser,
}



#[derive(Serialize, Deserialize)]
pub struct State {
    // Keeps track of GLOBAL files requested count 
    // and is used to assign IDs to newly requested files.
    file_count: u64,

    /// Keeps track of usernames vs. their principals.
    pub users: BTreeMap<Principal, User>,


    // /// Mapping between a user's principal and the list of files that are owned by the user.
    // pub file_owners: BTreeMap<Principal, Vec<u64>>,

    /// Mapping between a user's principal and the list of files that are shared with them.
    pub file_shares: BTreeMap<Principal, Vec<u64>>,

}

impl State {
    // pub(crate) fn generate_file_id(&mut self) -> u64 {
    //     // The file ID is an auto-incrementing integer.

    //     let file_id = self.file_count;
    //     self.file_count += 1;
    //     file_id
    // }

    fn new(rand_seed: &[u8]) -> Self {
        Self {
            file_count: 0,
            users: BTreeMap::new(),
            file_shares: BTreeMap::new(),
        }
    }


}

impl Default for State {
    fn default() -> Self {
        State::new(vec![0; 32].as_slice())
    }
}

/// A helper method to read the state.
///
/// Precondition: the state is already initialized.
pub fn with_state<R>(f: impl FnOnce(&State) -> R) -> R {
    STATE.with(|cell| f(&cell.borrow()))
}

/// A helper method to mutate the state.
///
/// Precondition: the state is already initialized.
pub fn with_state_mut<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|cell| f(&mut cell.borrow_mut()))
}


#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicUser {
    pub username: String,
    pub public_key: Vec<u8>,
    pub ic_principal: Principal,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GetUsersResponse {
    #[serde(rename = "permission_error")]
    PermissionError,
    #[serde(rename = "users")]
    Users(Vec<PublicUser>),
}



#[cfg(target_arch = "wasm32")]
pub fn get_time() -> u64 {
    ic_cdk::api::time()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_time() -> u64 {
    // This is used only in tests and we need a fixed value we can test against.
    12345
}

fn get_randomness_seed() -> Vec<u8> {
    // this is an array of u8 of length 8.
    let time_seed = ic_cdk::api::time().to_be_bytes();
    // we need to extend this to an array of size 32 by adding to it an array of size 24 full of 0s.
    let zeroes_arr: [u8; 24] = [0; 24];
    [&time_seed[..], &zeroes_arr[..]].concat()
}


#[update]
fn set_user(username: String, public_key: Vec<u8>, canister_id: Principal) -> SetUserResponse {
    if with_state(|s| crate::api::username_exists(s, username.clone())) {
        SetUserResponse::UsernameExists
    } else {
        let user = User {
            username,
            public_key,
            canister_id
        };
        with_state_mut(|s| crate::api::set_user_info(s, caller(), user));
        SetUserResponse::Ok
    }
}

#[query]
fn username_exists(username: String) -> bool {
    with_state(|s| crate::api::username_exists(s, username))
}

#[query]
fn who_am_i() -> WhoamiResponse {
    with_state(|s| match s.users.get(&ic_cdk::api::caller()) {
        None => WhoamiResponse::UnknownUser,
        Some(user) => WhoamiResponse::KnownUser(PublicUser {
            username: user.username.clone(),
            public_key: user.public_key.clone(),
            ic_principal: ic_cdk::api::caller(),
        }),
    })
}


#[query]
fn get_shared_files() -> Vec<u64/*PublicFileMetadata */> {
    with_state(|s| crate::api::get_shared_files(s, caller()))
}

// #[query]
// fn get_alias_info(alias: String) -> Result<AliasInfo, GetAliasInfoError> {
//     with_state(|s| crate::api::get_alias_info(s, alias))
// }

// #[update]
// fn upload_file(request: UploadFileRequest) -> Result<(), UploadFileError> {
//     with_state_mut(|s| {
//         crate::api::upload_file(
//             request.file_id,
//             request.file_content,
//             request.file_type,
//             request.owner_key,
//             request.num_chunks,
//             s,
//         )
//     })
// }

// #[update]
// fn upload_file_atomic(request: UploadFileAtomicRequest) -> u64 {
//     with_state_mut(|s| crate::api::upload_file_atomic(caller(), request, s))
// }

// #[update]
// fn upload_file_continue(request: UploadFileContinueRequest) {
//     with_state_mut(|s| crate::api::upload_file_continue(request, s))
// }

// #[update]
// fn request_file(request_name: String) -> String {
//     with_state_mut(|s| crate::api::request_file(caller(), request_name, s))
// }

// #[query]
// fn download_file(file_id: u64, chunk_id: u64) -> FileDownloadResponse {
//     with_state(|s| crate::api::download_file(s, file_id, chunk_id, caller()))
// }

// #[update]
// fn share_file(
//     user_id: Principal,
//     file_id: u64,
//     file_key_encrypted_for_user: Vec<u8>,
// ) -> FileSharingResponse {
//     with_state_mut(|s| {
//         crate::api::share_file(s, caller(), user_id, file_id, file_key_encrypted_for_user)
//     })
// }

// #[update]
// fn share_file_with_users(
//     user_id: Vec<Principal>,
//     file_id: u64,
//     file_key_encrypted_for_user: Vec<Vec<u8>>,
// ) {
//     with_state_mut(|s| {
//         for (id, key) in user_id.iter().zip(file_key_encrypted_for_user.iter()) {
//             crate::api::share_file(s, caller(), *id, file_id, key.clone());
//         }
//     });
// }

// #[update]
// fn revoke_share(user_id: Principal, file_id: u64) -> FileSharingResponse {
//     with_state_mut(|s| crate::api::revoke_share(s, caller(), user_id, file_id))
// }

#[query]
fn get_users() -> GetUsersResponse {
    with_state(|s| crate::api::get_users(s, caller()))
}

#[pre_upgrade]
fn pre_upgrade() {
    crate::upgrade::pre_upgrade();
}

#[post_upgrade]
fn post_upgrade() {
    crate::upgrade::post_upgrade();
}

/// GetRandom fixup to allow getrandom compilation.
/// A getrandom implementation that always fails
///
/// This is a workaround for the fact that the `getrandom` crate does not compile
/// for the `wasm32-unknown-ic` target. This is a dummy implementation that always
/// fails with `Error::UNSUPPORTED`.
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    _dest: *mut u8,
    _len: usize,
) -> Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}

// getrandom::register_custom_getrandom!(getrandom_always_fail);

ic_cdk::export_candid!();


// Implement State, helper functions, and methods as in the original lib.rs
// Move orchestrator-specific methods (set_user, request_file, etc.) here

