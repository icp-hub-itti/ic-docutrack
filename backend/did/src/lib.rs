pub mod orchestrator;

use candid::{CandidType, Decode, Encode};
use ic_stable_structures::Storable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, CandidType)]
pub struct State {
    pub name: String,
    pub value: u64,
}

impl Storable for State {
    const BOUND: ic_stable_structures::storable::Bound =
        ic_stable_structures::storable::Bound::Unbounded;

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Encode!(self).unwrap().into()
    }
}//! Candid types for ic-docutrack canisters

#[rustfmt::skip]
#[allow(clippy::all)]
#[allow(deprecated)]
pub mod orbit_station;
