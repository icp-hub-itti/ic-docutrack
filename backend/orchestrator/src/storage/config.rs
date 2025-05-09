use std::cell::RefCell;

use candid::Principal;
use did::StorablePrincipal;
use ic_stable_structures::memory_manager::VirtualMemory;
use ic_stable_structures::{DefaultMemoryImpl, StableCell};

use super::memory::{MEMORY_MANAGER, ORBIT_STATION_ADMIN_MEMORY_ID, ORBIT_STATION_MEMORY_ID};
use crate::utils::trap;

thread_local! {
    /// Orbit station
    static ORBIT_STATION: RefCell<StableCell<StorablePrincipal, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(ORBIT_STATION_MEMORY_ID)), Principal::anonymous().into()).unwrap()
    );

    /// Orbit station admin
    static ORBIT_STATION_ADMIN: RefCell<StableCell<String, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(ORBIT_STATION_ADMIN_MEMORY_ID)), String::default()).unwrap()
    );
}

/// Canister configuration
pub struct Config;

impl Config {
    /// Get the orbit station [`Principal`]
    pub fn get_orbit_station() -> Principal {
        ORBIT_STATION.with_borrow(|cell| cell.get().0)
    }

    /// Set the orbit station [`Principal`]
    pub fn set_orbit_station(principal: Principal) {
        if let Err(err) = ORBIT_STATION.with_borrow_mut(|cell| cell.set(principal.into())) {
            trap(format!("Failed to set orbit station: {:?}", err));
        }
    }

    /// Get the orbit station admin
    pub fn get_orbit_station_admin() -> String {
        ORBIT_STATION_ADMIN.with_borrow(|cell| cell.get().clone())
    }

    /// Set the orbit station admin
    pub fn set_orbit_station_admin(admin: String) {
        if let Err(err) = ORBIT_STATION_ADMIN.with_borrow_mut(|cell| cell.set(admin)) {
            trap(format!("Failed to set orbit station admin: {:?}", err));
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_orbit_station() {
        let principal = Principal::from_slice(&[1; 29]);
        Config::set_orbit_station(principal);
        assert_eq!(Config::get_orbit_station(), principal);
    }

    #[test]
    fn test_orbit_station_admin() {
        let admin = "admin".to_string();
        Config::set_orbit_station_admin(admin.clone());
        assert_eq!(Config::get_orbit_station_admin(), admin);
    }
}
