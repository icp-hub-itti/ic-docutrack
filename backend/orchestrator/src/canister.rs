mod create_user;

use candid::Principal;
use create_user::CreateUserStateMachine;
use did::orchestrator::{
    FileId, GetUsersResponse, MAX_USERNAME_SIZE, OrchestratorInstallArgs, PublicKey, PublicUser,
    RetryUserCanisterCreationResponse, RevokeShareFileResponse, SetUserResponse, ShareFileResponse,
    SharedFilesResponse, User, UserCanisterResponse, WhoamiResponse,
};

use crate::storage::config::Config;
use crate::storage::shared_files::SharedFilesStorage;
use crate::storage::user_canister::{UserCanisterCreateState, UserCanisterStorage};
use crate::storage::users::UserStorage;
use crate::utils::{msg_caller, trap};

/// API for Business Logic
pub struct Canister;

impl Canister {
    /// Initialize the canister with the given arguments.
    pub fn init(args: OrchestratorInstallArgs) {
        let OrchestratorInstallArgs::Init(args) = args else {
            trap("Invalid arguments");
        };

        Config::set_orbit_station(args.orbit_station);
        Config::set_orbit_station_admin(args.orbit_station_admin);
    }

    /// Get the users from the storage as [`GetUsersResponse`].
    ///
    /// If the caller is anonymous, it returns [`GetUsersResponse::PermissionError`].
    ///
    /// FIXME: this function is going to exhaust memory when called if we don't introduce pagination.
    /// There is already a task for it in the backlog.
    /// FIXME: this function should be protected.
    pub fn get_users() -> GetUsersResponse {
        let caller = msg_caller();
        if caller == Principal::anonymous() {
            return GetUsersResponse::PermissionError;
        }

        UserStorage::get_users()
            .into_iter()
            .map(|(principal, user)| PublicUser::new(user, principal))
            .collect::<Vec<_>>()
            .into()
    }

    /// Retry the user canister creation for the current caller.
    ///
    /// # Returns
    ///
    /// - [`RetryUserCanisterCreationResponse::Ok`] if the user canister creation is retried.
    /// - [`RetryUserCanisterCreationResponse::Created`] if the user canister already exists.
    /// - [`RetryUserCanisterCreationResponse::AnonymousCaller`]: The caller is anonymous.
    /// - [`RetryUserCanisterCreationResponse::CreationPending`]: The user canister creation is already in progress.
    /// - [`RetryUserCanisterCreationResponse::UserNotFound`]: The user doesn't exist. In that case, the caller should call `set_user` first.
    pub fn retry_user_canister_creation() -> RetryUserCanisterCreationResponse {
        let caller = msg_caller();
        if caller == Principal::anonymous() {
            return RetryUserCanisterCreationResponse::AnonymousCaller;
        }

        // check if the user exists
        if UserStorage::get_user(&caller).is_none() {
            return RetryUserCanisterCreationResponse::UserNotFound;
        }

        // check if the user canister already exists
        if let Some(canister) = UserCanisterStorage::get_user_canister(caller) {
            return RetryUserCanisterCreationResponse::Created(canister);
        }

        // check the current state of the user canister creation
        match UserCanisterStorage::get_create_state(caller) {
            Some(UserCanisterCreateState::Ok { user_canister }) => {
                RetryUserCanisterCreationResponse::Created(user_canister)
            }
            Some(UserCanisterCreateState::Failed { .. }) | None => {
                if cfg!(target_family = "wasm") {
                    CreateUserStateMachine::start(Config::get_orbit_station(), caller);
                }
                RetryUserCanisterCreationResponse::Ok
            }
            Some(_) => RetryUserCanisterCreationResponse::CreationPending,
        }
    }

    /// Revoke the share of a file for a user.
    ///
    /// # Returns
    ///
    /// - [`RevokeShareFileResponse::Ok`] if the file was unshared successfully.
    /// - [`RevokeShareFileResponse::NoSuchUser`] if the user doesn't exist.
    /// - [`RevokeShareFileResponse::Unauthorized`] if the caller is not a user canister.
    pub fn revoke_share_file(user: Principal, file_id: FileId) -> RevokeShareFileResponse {
        let user_canister = msg_caller();
        // check if the caller is a user canister
        if !UserCanisterStorage::is_user_canister(user_canister) {
            return RevokeShareFileResponse::Unauthorized;
        }

        // Revoke share for the user
        SharedFilesStorage::revoke_share(user, user_canister, file_id);

        RevokeShareFileResponse::Ok
    }

    /// Set a new user in the storage.
    ///
    /// # Returns
    ///
    /// - [`SetUserResponse::Ok`] if the user was set successfully.
    /// - [`SetUserResponse::AnonymousCaller`] if the caller is anonymous.
    /// - [`SetUserResponse::UsernameTooLong`] if the username is too long.
    /// - [`SetUserResponse::UsernameExists`] if the username already exists.
    /// - [`SetUserResponse::CallerHasAlreadyAUser`] if the caller already has a user.
    pub fn set_user(username: String, public_key: PublicKey) -> SetUserResponse {
        // Check if the caller is anonymous.
        let caller = msg_caller();
        if caller == Principal::anonymous() {
            return SetUserResponse::AnonymousCaller;
        }

        // Check if the username is too long.
        if username.len() > MAX_USERNAME_SIZE {
            return SetUserResponse::UsernameTooLong;
        }

        // check if username already exists
        if UserStorage::username_exists(&username) {
            return SetUserResponse::UsernameExists;
        }

        // check if the caller already has a user
        if UserStorage::get_user(&caller).is_some() {
            return SetUserResponse::CallerHasAlreadyAUser;
        }

        // Add the user to the storage and return Ok.
        UserStorage::add_user(
            caller,
            User {
                username,
                public_key,
            },
        );

        // start state machine to create user canister
        if cfg!(target_family = "wasm") {
            CreateUserStateMachine::start(Config::get_orbit_station(), caller);
        }

        SetUserResponse::Ok
    }

    /// Share a file with a user.
    ///
    /// # Returns
    ///
    /// - [`ShareFileResponse::Ok`] if the file was shared successfully.
    /// - [`ShareFileResponse::NoSuchUser`] if the user doesn't exist.
    /// - [`ShareFileResponse::Unauthorized`] if the caller is not a user canister.
    pub fn share_file(user: Principal, file_id: FileId) -> ShareFileResponse {
        Self::share_file_with_users(vec![user], file_id)
    }

    /// Share a file with many users.
    ///
    /// # Returns
    ///
    /// - [`ShareFileResponse::Ok`] if the file was shared successfully.
    /// - [`ShareFileResponse::NoSuchUser`] if the user doesn't exist.
    /// - [`ShareFileResponse::Unauthorized`] if the caller is not a user canister.
    pub fn share_file_with_users(users: Vec<Principal>, file_id: FileId) -> ShareFileResponse {
        let user_canister = msg_caller();
        // check if the caller is a user canister
        if !UserCanisterStorage::is_user_canister(user_canister) {
            return ShareFileResponse::Unauthorized;
        }

        // check if all the users exist
        if let Some(no_such_user) = users
            .iter()
            .find(|user| UserStorage::get_user(user).is_none())
        {
            return ShareFileResponse::NoSuchUser(*no_such_user);
        }

        // share the file with all the users
        for user in users {
            SharedFilesStorage::share_file(user, user_canister, file_id);
        }

        ShareFileResponse::Ok
    }

    /// Returns the list of shared files for the caller.
    ///
    /// # Returns
    ///
    /// - [`SharedFilesResponse::AnonymousUser`] if the caller is anonymous.
    /// - [`SharedFilesResponse::NoSuchUser`] if the user doesn't exist.
    /// - [`SharedFilesResponse::SharedFiles`] if the user exists and has shared files.
    pub fn shared_files() -> SharedFilesResponse {
        let caller = msg_caller();
        if caller == Principal::anonymous() {
            return SharedFilesResponse::AnonymousUser;
        }

        // check if the user exists
        if UserStorage::get_user(&caller).is_none() {
            return SharedFilesResponse::NoSuchUser;
        }

        SharedFilesResponse::SharedFiles(SharedFilesStorage::get_shared_files(caller))
    }

    /// Checks whether a given username exists in the storage.
    pub fn username_exists(username: String) -> bool {
        UserStorage::username_exists(&username)
    }

    /// Get user canister information for the current caller.
    ///
    /// # Returns
    ///
    /// - [`UserCanisterResponse::AnonymousCaller`] if the caller is anonymous.
    /// - [`UserCanisterResponse::Ok`] if the user canister is created and ready to use.
    /// - [`UserCanisterResponse::CreationPending`] if the user canister is being created.
    /// - [`UserCanisterResponse::CreationFailed`] if the user canister creation failed.
    pub fn user_canister() -> UserCanisterResponse {
        let caller = msg_caller();
        if caller == Principal::anonymous() {
            return UserCanisterResponse::AnonymousCaller;
        }

        if let Some(canister) = UserCanisterStorage::get_user_canister(caller) {
            return UserCanisterResponse::Ok(canister);
        }

        // otherwise check if it failed or it is pending
        UserCanisterStorage::get_create_state(caller)
            .map(|state| match state {
                UserCanisterCreateState::Failed { reason } => {
                    UserCanisterResponse::CreationFailed { reason }
                }
                _ => UserCanisterResponse::CreationPending,
            })
            .unwrap_or(UserCanisterResponse::Uninitialized)
    }

    /// Get [`WhoamiResponse`] for the current caller.
    ///
    /// # Returns
    ///
    /// - [`WhoamiResponse::UnknownUser`] if the caller is anonymous or doesn't exist.
    /// - [`WhoamiResponse::KnownUser`] if the caller exists.
    pub fn whoami() -> WhoamiResponse {
        let caller = msg_caller();
        if caller == Principal::anonymous() {
            return WhoamiResponse::UnknownUser;
        }

        UserStorage::get_user(&caller)
            .map(|user| PublicUser::new(user, caller))
            .map(WhoamiResponse::from)
            .unwrap_or(WhoamiResponse::UnknownUser)
    }
}

#[cfg(test)]
mod test {

    use std::collections::HashMap;

    use did::orchestrator::{OrchestratorInitArgs, User};

    use super::*;

    #[test]
    fn test_should_init_canister() {
        let orbit_station = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        Canister::init(OrchestratorInstallArgs::Init(OrchestratorInitArgs {
            orbit_station,
            orbit_station_admin: "admin".to_string(),
        }));

        assert_eq!(Config::get_orbit_station(), orbit_station);
    }

    #[test]
    fn test_should_get_users() {
        init_canister();

        // setup user
        let principal = msg_caller();
        UserStorage::add_user(
            principal,
            User {
                username: "test_user".to_string(),
                public_key: [1; 32],
            },
        );

        // get users
        let response = Canister::get_users();
        assert_eq!(
            response,
            GetUsersResponse::Users(vec![PublicUser {
                username: "test_user".to_string(),
                public_key: [1; 32],
                ic_principal: principal,
            }])
        );
    }

    #[test]
    fn test_should_retry_user_canister_creation() {
        init_canister();

        // let's setup a user
        let principal = msg_caller();
        UserStorage::add_user(
            principal,
            User {
                username: "test_user".to_string(),
                public_key: [1; 32],
            },
        );

        // of course this won't start the state machine on test unit; let's set the state to failed
        UserCanisterStorage::set_create_state(
            principal,
            UserCanisterCreateState::Failed {
                reason: "test".to_string(),
            },
        );

        // we can retry now :D
        let response = Canister::retry_user_canister_creation();
        assert_eq!(response, RetryUserCanisterCreationResponse::Ok);
    }

    #[test]
    fn test_should_not_retry_user_canister_creation_if_user_does_not_exist() {
        init_canister();

        // let's setup another user
        UserStorage::add_user(
            Principal::management_canister(),
            User {
                username: "test_user".to_string(),
                public_key: [1; 32],
            },
        );

        // user does not exist
        let response = Canister::retry_user_canister_creation();
        assert_eq!(response, RetryUserCanisterCreationResponse::UserNotFound);
    }

    #[test]
    fn test_should_not_retry_if_user_canister_exists() {
        init_canister();

        // let's setup a user
        let principal = msg_caller();
        UserStorage::add_user(
            principal,
            User {
                username: "test_user".to_string(),
                public_key: [1; 32],
            },
        );

        // let's set the user canister
        let user_canister = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        UserCanisterStorage::set_user_canister(principal, user_canister);

        // canister already exists
        let response = Canister::retry_user_canister_creation();
        assert_eq!(
            response,
            RetryUserCanisterCreationResponse::Created(user_canister)
        );
    }

    #[test]
    fn test_should_not_retry_if_pending() {
        init_canister();

        // let's setup a user
        let principal = msg_caller();
        UserStorage::add_user(
            principal,
            User {
                username: "test_user".to_string(),
                public_key: [1; 32],
            },
        );

        // let's set the user canister creation state to something pending
        UserCanisterStorage::set_create_state(principal, UserCanisterCreateState::CreateCanister);

        // canister already exists
        let response = Canister::retry_user_canister_creation();
        assert_eq!(response, RetryUserCanisterCreationResponse::CreationPending);
    }

    #[test]
    fn test_should_register_user_if_valid() {
        init_canister();

        // setup user
        let principal = msg_caller();
        let username = "test_user".to_string();
        let public_key = [1; 32];

        // register user
        let response = Canister::set_user(username.clone(), public_key);
        assert_eq!(response, SetUserResponse::Ok);

        // check if user exists
        let user = UserStorage::get_user(&principal).unwrap();
        assert_eq!(user.username, username);
        assert_eq!(user.public_key, public_key);
    }

    #[test]
    fn test_should_not_add_user_if_username_too_long() {
        init_canister();

        // setup user
        let principal = msg_caller();
        let username = "a".repeat(MAX_USERNAME_SIZE + 1);
        let public_key = [1; 32];

        // register user
        let response = Canister::set_user(username.clone(), public_key);
        assert_eq!(response, SetUserResponse::UsernameTooLong);

        // check if user does not exist
        let user = UserStorage::get_user(&principal);
        assert!(user.is_none());
    }

    #[test]
    fn test_should_not_add_user_if_caller_has_already_a_user() {
        init_canister();

        // setup user
        let username = "test_user".to_string();
        let public_key = [1; 32];

        // register user
        let response = Canister::set_user(username.clone(), public_key);
        assert_eq!(response, SetUserResponse::Ok);

        // try another username
        let response = Canister::set_user("foo".to_string(), public_key);
        assert_eq!(response, SetUserResponse::CallerHasAlreadyAUser);
    }

    #[test]
    fn test_should_tell_if_username_exists() {
        init_canister();

        // setup user
        let principal = msg_caller();
        UserStorage::add_user(
            principal,
            User {
                username: "test_user".to_string(),
                public_key: [1; 32],
            },
        );

        // check if username exists
        let exists = Canister::username_exists("test_user".to_string());
        assert!(exists);

        // check if non-existing username exists
        let exists = Canister::username_exists("non_existing_user".to_string());
        assert!(!exists);
    }

    #[test]
    fn test_should_tell_whoami() {
        init_canister();

        // setup user
        let principal = msg_caller();
        UserStorage::add_user(
            principal,
            User {
                username: "test_user".to_string(),
                public_key: [1; 32],
            },
        );

        // get whoami
        let whoami = Canister::whoami();
        assert_eq!(
            whoami,
            WhoamiResponse::KnownUser(PublicUser {
                username: "test_user".to_string(),
                public_key: [1; 32],
                ic_principal: principal,
            })
        );
    }

    #[test]
    fn test_should_return_shared_files() {
        init_canister();

        // setup user
        let principal = msg_caller();
        UserStorage::add_user(
            principal,
            User {
                username: "test_user".to_string(),
                public_key: [1; 32],
            },
        );

        // insert shared files
        let file_id = 1;
        let user_canister = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();

        SharedFilesStorage::share_file(principal, user_canister, file_id);

        let mut expected = HashMap::new();
        expected.insert(user_canister, vec![file_id].into_iter().collect());

        // get shared files
        let shared_files = Canister::shared_files();
        assert_eq!(shared_files, SharedFilesResponse::SharedFiles(expected));
    }

    #[test]
    fn test_should_return_error_on_shared_files_unexisting_user() {
        init_canister();

        // get shared files
        let shared_files = Canister::shared_files();
        assert_eq!(shared_files, SharedFilesResponse::NoSuchUser);
    }

    #[test]
    fn test_should_revoke_shared_file() {
        init_canister();

        // insert user canister
        let user_canister = msg_caller();
        let user = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        UserCanisterStorage::set_user_canister(user, user_canister);

        // revoke share
        let file_id = 1;
        SharedFilesStorage::share_file(user, user_canister, file_id);
        let response = Canister::revoke_share_file(user, file_id);
        assert_eq!(response, RevokeShareFileResponse::Ok);

        // check if the file is revoked
        let shared_files = SharedFilesStorage::get_shared_files(user);
        assert_eq!(shared_files.len(), 0);
    }

    #[test]
    fn test_should_not_revoke_shared_file_if_caller_is_not_a_user_canister() {
        init_canister();

        // insert user canister
        let user_canister = msg_caller();
        let user = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();

        // revoke share
        let file_id = 1;
        SharedFilesStorage::share_file(user, user_canister, file_id);
        let response = Canister::revoke_share_file(user, file_id);
        assert_eq!(response, RevokeShareFileResponse::Unauthorized);

        // check if the file is NOT revoked
        let shared_files = SharedFilesStorage::get_shared_files(user);
        assert_eq!(shared_files.len(), 1);
    }

    #[test]
    fn test_should_share_file() {
        init_canister();

        // insert user canister
        let user_canister = msg_caller();
        let user = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();

        // set user canister
        UserCanisterStorage::set_user_canister(user, user_canister);

        // create user
        let alice = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        UserStorage::add_user(
            alice,
            User {
                username: "test_user".to_string(),
                public_key: [1; 32],
            },
        );

        // share file
        let file_id = 1;
        let response = Canister::share_file(alice, file_id);
        assert_eq!(response, ShareFileResponse::Ok);

        // check if the file is shared
        let shared_files = SharedFilesStorage::get_shared_files(alice);
        assert_eq!(shared_files.len(), 1);
    }

    #[test]
    fn test_should_not_share_if_not_called_by_user_canister() {
        init_canister();

        let user = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();

        // share file
        let file_id = 1;
        let response = Canister::share_file(user, file_id);
        assert_eq!(response, ShareFileResponse::Unauthorized);

        // check if the file is NOT shared
        let shared_files = SharedFilesStorage::get_shared_files(user);
        assert_eq!(shared_files.len(), 0);
    }

    #[test]
    fn test_should_not_share_if_user_does_not_exist() {
        init_canister();

        // insert user canister
        let user_canister = msg_caller();
        let user = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();

        // set user canister
        UserCanisterStorage::set_user_canister(user, user_canister);

        let alice = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();

        // share file
        let file_id = 1;
        let response = Canister::share_file(alice, file_id);
        assert_eq!(response, ShareFileResponse::NoSuchUser(alice));

        // check if the file is NOT shared
        let shared_files = SharedFilesStorage::get_shared_files(alice);
        assert_eq!(shared_files.len(), 0);
    }

    fn init_canister() {
        let orbit_station = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        Canister::init(OrchestratorInstallArgs::Init(OrchestratorInitArgs {
            orbit_station,
            orbit_station_admin: "admin".to_string(),
        }));
    }
}
