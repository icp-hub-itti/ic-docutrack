mod create_state;

use std::cell::RefCell;

use candid::Principal;
use did::StorablePrincipal;
use ic_stable_structures::memory_manager::VirtualMemory;
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap};

pub use self::create_state::UserCanisterCreateState;
use crate::storage::memory::{
    MEMORY_MANAGER, USER_CANISTER_CREATE_STATES_MEMORY_ID, USER_CANISTERS_MEMORY_ID,
};

thread_local! {
    /// User canisters
    static USER_CANISTERS: RefCell<StableBTreeMap<StorablePrincipal, StorablePrincipal, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(USER_CANISTERS_MEMORY_ID)))
    );

    /// Users storage map
    static USER_CANISTER_CREATE_STATES: RefCell<StableBTreeMap<StorablePrincipal, UserCanisterCreateState, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(USER_CANISTER_CREATE_STATES_MEMORY_ID)))
    );
}

/// User canister storage to access user canisters and their create states
pub struct UserCanisterStorage;

impl UserCanisterStorage {
    /// Initialize a user canister creation.
    pub fn init_create_state(principal: Principal) {
        USER_CANISTER_CREATE_STATES.with_borrow_mut(|states| {
            states.insert(principal.into(), UserCanisterCreateState::CreateCanister)
        });
    }

    /// Get the [`UserCanisterCreateState`] for a certain user.
    pub fn get_create_state(principal: Principal) -> Option<UserCanisterCreateState> {
        USER_CANISTER_CREATE_STATES
            .with_borrow(|states| states.get(&StorablePrincipal::from(principal)).clone())
    }

    /// Update the [`UserCanisterCreateState`] for a certain user.
    pub fn set_create_state(principal: Principal, state: UserCanisterCreateState) {
        USER_CANISTER_CREATE_STATES.with_borrow_mut(|states| {
            states.insert(principal.into(), state);
        });
    }

    /// Set the user canister for a certain user.
    ///
    /// Setting the user canister will remove the current user creation state.
    pub fn set_user_canister(principal: Principal, user_canister: Principal) {
        USER_CANISTERS.with_borrow_mut(|canisters| {
            canisters.insert(principal.into(), user_canister.into());
        });

        USER_CANISTER_CREATE_STATES.with_borrow_mut(|states| {
            states.remove(&StorablePrincipal::from(principal));
        });
    }

    /// Get the user canister for a certain user.
    pub fn get_user_canister(principal: Principal) -> Option<Principal> {
        USER_CANISTERS
            .with_borrow(|canisters| canisters.get(&StorablePrincipal::from(principal)))
            .map(|p| p.0)
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_init_create_state() {
        let principal = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        UserCanisterStorage::init_create_state(principal);

        assert_eq!(
            UserCanisterStorage::get_create_state(principal),
            Some(UserCanisterCreateState::CreateCanister)
        );
    }

    #[test]
    fn test_should_set_create_state() {
        let principal = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        UserCanisterStorage::init_create_state(principal);

        UserCanisterStorage::set_create_state(
            principal,
            UserCanisterCreateState::WaitForCreateCanisterSchedule {
                scheduled_at: "2023-10-01T00:00:00Z".to_string(),
                request_id: "request_id".to_string(),
            },
        );

        assert_eq!(
            UserCanisterStorage::get_create_state(principal),
            Some(UserCanisterCreateState::WaitForCreateCanisterSchedule {
                scheduled_at: "2023-10-01T00:00:00Z".to_string(),
                request_id: "request_id".to_string(),
            })
        );
    }

    #[test]
    fn test_should_set_user_canister() {
        let principal = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        let user_canister = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        UserCanisterStorage::set_user_canister(principal, user_canister);

        assert_eq!(
            UserCanisterStorage::get_user_canister(principal),
            Some(user_canister)
        );

        assert_eq!(UserCanisterStorage::get_create_state(principal), None);
    }
}
