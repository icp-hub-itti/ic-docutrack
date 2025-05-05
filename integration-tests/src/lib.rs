use candid::{CandidType, Principal};
use serde::de::DeserializeOwned;

pub mod actor;
#[cfg(feature = "pocket-ic")]
mod pocket_ic;
mod wasm;

#[cfg(feature = "pocket-ic")]
pub use self::pocket_ic::PocketIcTestEnv;

pub trait TestEnv {
    fn query<R>(
        &self,
        canister: Principal,
        caller: Principal,
        method: &str,
        payload: Vec<u8>,
    ) -> impl Future<Output = anyhow::Result<R>>
    where
        R: DeserializeOwned + CandidType;

    fn update<R>(
        &self,
        canister: Principal,
        caller: Principal,
        method: &str,
        payload: Vec<u8>,
    ) -> impl Future<Output = anyhow::Result<R>>
    where
        R: DeserializeOwned + CandidType;

    /// Admin principal id
    fn admin(&self) -> Principal;

    /// Backend canister id
    fn orchestrator(&self) -> Principal;
    fn user_canister1(&self) -> Principal;

    /// Orbit station canister id
    fn orbit_station(&self) -> Principal;

    /// Uuid of the station admin
    fn station_admin(&self) -> String;
}
