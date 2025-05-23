use std::cell::RefCell;
use std::collections::HashMap;

use candid::Principal;
use did::StorablePrincipal;
use did::orchestrator::User;
use ic_stable_structures::memory_manager::VirtualMemory;
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap};

use crate::storage::memory::{MEMORY_MANAGER, USER_STORAGE_MEMORY_ID, USERNAMES_MEMORY_ID};

thread_local! {
    /// Users storage map
    static USERS_STORAGE: RefCell<StableBTreeMap<StorablePrincipal, User, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(USER_STORAGE_MEMORY_ID)))
    );

    /// Usernames storage map.
    ///
    /// We use another map to index usernames, because we need to expose an endpoint to check if a username exists.
    /// And checking if a username exists in the users storage is not efficient. O(n), while checking in the
    /// usernames storage is O(log(n)).
    static USERNAMES: RefCell<StableBTreeMap<String, (), VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(USERNAMES_MEMORY_ID)))
    );

}

/// Accessor to the users storage
fn with_users_storage<T, F>(f: F) -> T
where
    F: FnOnce(&StableBTreeMap<StorablePrincipal, User, VirtualMemory<DefaultMemoryImpl>>) -> T,
{
    USERS_STORAGE.with_borrow(|users| f(users))
}

/// Immutable accessor to a user
fn with_user<T, F>(principal: &Principal, f: F) -> Option<T>
where
    F: FnOnce(User) -> T,
{
    USERS_STORAGE.with_borrow(|users| users.get(&StorablePrincipal::from(*principal)).map(f))
}

/// Public API for the user storage
pub struct UserStorage;

impl UserStorage {
    /// Get a user by principal
    pub fn get_user(principal: &Principal) -> Option<User> {
        with_user(principal, |user| user)
    }

    /// Get all users in a range
    pub fn get_users_in_range(offset: u64, limit: u64) -> HashMap<Principal, User> {
        with_users_storage(|users| {
            users
                .iter()
                .skip(offset as usize)
                .take(limit as usize)
                .map(|(principal, user)| (principal.0, user.clone()))
                .collect()
        })
    }

    /// Get the number of users in the storage
    pub fn len() -> u64 {
        USERS_STORAGE.with_borrow(|users| users.len())
    }

    /// Add a user to the storage.
    ///
    /// It adds the username to the usernames storage and the user to the users storage.
    ///
    /// # Panics
    ///
    /// If the principal is anonymous, it will panic.
    pub fn add_user(principal: Principal, user: User) {
        if principal == Principal::anonymous() {
            crate::utils::trap("Cannot add anonymous user");
        }

        USERNAMES.with_borrow_mut(|usernames| {
            usernames.insert(user.username.clone(), ());
        });

        USERS_STORAGE.with_borrow_mut(|users| {
            users.insert(StorablePrincipal::from(principal), user);
        });
    }

    /// Checks whether a username exists in the storage
    pub fn username_exists(username: &String) -> bool {
        USERNAMES.with_borrow(|usernames| usernames.contains_key(username))
    }
}

#[cfg(test)]
mod test {

    use did::orchestrator::PublicKey;

    use super::*;

    #[test]
    fn test_should_insert_and_read_users() {
        let user = User {
            username: "test_user".to_string(),
            public_key: PublicKey::try_from(vec![1; 32]).expect("invalid public key"),
        };
        let principal = Principal::from_slice(&[1; 29]);

        // Add user
        UserStorage::add_user(principal, user.clone());
        // Get user
        let retrieved_user = UserStorage::get_user(&principal).unwrap();
        // Check if the retrieved user matches the original user
        assert_eq!(retrieved_user, user);

        // add another user
        let user2 = User {
            username: "test_user2".to_string(),
            public_key: PublicKey::try_from(vec![2; 32]).expect("invalid public key"),
        };
        let principal2 = Principal::from_slice(&[2; 29]);
        UserStorage::add_user(principal2, user2.clone());

        // Get all users
        let all_users = UserStorage::get_users_in_range(0, u64::MAX);
        // Check if the length of all users is 2
        assert_eq!(all_users.len(), 2);
        // Check if the retrieved user is in the list of all users
        assert_eq!(all_users.get(&principal), Some(&user));
        assert_eq!(all_users.get(&principal2), Some(&user2));
    }

    #[test]
    #[should_panic = "Cannot add anonymous user"]
    fn test_should_panic_when_adding_anonymous_user() {
        let user = User {
            username: "test_user".to_string(),
            public_key: PublicKey::try_from(vec![1; 32]).expect("invalid public key"),
        };
        let principal = Principal::anonymous();

        // Add user
        UserStorage::add_user(principal, user);
    }

    #[test]
    fn test_should_tell_whether_username_exists() {
        let user = User {
            username: "test_user".to_string(),
            public_key: PublicKey::try_from(vec![1; 32]).expect("invalid public key"),
        };
        let principal = Principal::from_slice(&[1; 29]);

        // Add user
        UserStorage::add_user(principal, user.clone());

        // Check if the username exists
        assert!(UserStorage::username_exists(&"test_user".to_string()));
        assert!(!UserStorage::username_exists(
            &"non_existent_user".to_string()
        ));
    }
}
