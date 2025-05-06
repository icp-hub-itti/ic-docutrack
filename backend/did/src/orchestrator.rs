#![allow(dead_code, unused_imports)]
use candid::{self, CandidType, Deserialize};



#[derive(CandidType, Deserialize,Debug)]
pub enum SetUserResponse {
    Ok,
    UsernameExists,
}