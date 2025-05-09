use candid::Principal;
use did::orbit_station::TimestampRfc3339;
use ic_stable_structures::Storable;
use ic_stable_structures::storable::Bound;

use crate::utils::trap;

const OP_CREATE_CANISTER: u8 = 0;
const OP_WAIT_FOR_CREATE_CANISTER_SCHEDULE: u8 = 1;
const OP_WAIT_FOR_CREATE_CANISTER_RESULT: u8 = 2;
const OP_INSTALL_CANISTER: u8 = 3;
const OP_WAIT_FOR_INSTALL_CANISTER_SCHEDULE: u8 = 4;
const OP_WAIT_FOR_INSTALL_CANISTER_RESULT: u8 = 5;
const OP_OK: u8 = 6;
const OP_FAILED: u8 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserCanisterCreateState {
    /// Send a request to the orbit station to create the user canister.
    CreateCanister,
    /// Wait for the orbit station to start scheduled canister creation.
    /// It can mutate to [`UserCanisterCreateState::WaitForCreateCanisterResult`] when executing.
    WaitForCreateCanisterSchedule {
        scheduled_at: TimestampRfc3339,
        request_id: String,
    },
    /// Wait for the create canister operation to finish.
    /// This state mutates to [`UserCanisterCreateState::WaitForCreateCanisterSchedule`] when scheduled.
    WaitForCreateCanisterResult { request_id: String },
    /// Send a request to Install canister on the user canister.
    InstallCanister { user_canister: Principal },
    /// Wait for the orbit station to start scheduled canister installation.
    /// It can mutate to [`UserCanisterCreateState::WaitForInstallCanisterResult`] when executing.
    WaitForInstallCanisterSchedule {
        user_canister: Principal,
        scheduled_at: TimestampRfc3339,
        request_id: String,
    },
    /// Wait for the install canister operation to finish.
    /// This state mutates to [`UserCanisterCreateState::WaitForInstallCanisterSchedule`] when scheduled.
    WaitForInstallCanisterResult {
        user_canister: Principal,
        request_id: String,
    },
    /// The user canister is created and installed.
    Ok { user_canister: Principal },
    /// The user canister creation failed.
    Failed { reason: String },
}

impl Storable for UserCanisterCreateState {
    const BOUND: Bound = Bound::Bounded {
        max_size: 512,
        is_fixed_size: false,
    };

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        if bytes.is_empty() {
            trap("Failed to decode UserCanisterCreateState: empty bytes");
        }
        // read op code
        let op_code = bytes[0];
        match op_code {
            OP_CREATE_CANISTER => Self::decode_create_canister(),
            OP_WAIT_FOR_CREATE_CANISTER_SCHEDULE => {
                Self::decode_wait_for_create_canister_schedule(&bytes[1..])
            }
            OP_WAIT_FOR_CREATE_CANISTER_RESULT => {
                Self::decode_wait_for_create_canister_result(&bytes[1..])
            }
            OP_INSTALL_CANISTER => Self::decode_install_canister(&bytes[1..]),
            OP_WAIT_FOR_INSTALL_CANISTER_SCHEDULE => {
                Self::decode_wait_for_install_canister_schedule(&bytes[1..])
            }
            OP_WAIT_FOR_INSTALL_CANISTER_RESULT => {
                Self::decode_wait_for_install_canister_result(&bytes[1..])
            }
            OP_OK => Self::decode_ok(&bytes[1..]),
            OP_FAILED => Self::decode_failed(&bytes[1..]),
            _ => trap("Failed to decode UserCanisterCreateState: invalid operation code"),
        }
    }

    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        match self {
            UserCanisterCreateState::CreateCanister => Self::encode_create_canister().into(),
            UserCanisterCreateState::WaitForCreateCanisterSchedule {
                scheduled_at,
                request_id,
            } => Self::encode_wait_for_create_canister_schedule(scheduled_at, request_id).into(),
            UserCanisterCreateState::WaitForCreateCanisterResult { request_id } => {
                Self::encode_wait_for_create_canister_result(request_id).into()
            }
            UserCanisterCreateState::InstallCanister { user_canister } => {
                Self::encode_install_canister(*user_canister).into()
            }
            UserCanisterCreateState::WaitForInstallCanisterSchedule {
                user_canister,
                scheduled_at,
                request_id,
            } => Self::encode_wait_for_install_canister_schedule(
                *user_canister,
                scheduled_at,
                request_id,
            )
            .into(),
            UserCanisterCreateState::WaitForInstallCanisterResult {
                user_canister,
                request_id,
            } => Self::encode_wait_for_install_canister_result(*user_canister, request_id).into(),
            UserCanisterCreateState::Ok { user_canister } => Self::encode_ok(*user_canister).into(),
            UserCanisterCreateState::Failed { reason } => Self::encode_failed(reason).into(),
        }
    }
}

impl UserCanisterCreateState {
    /// Encode variant for [`UserCanisterCreateState::CreateCanister`].
    fn encode_create_canister() -> Vec<u8> {
        vec![OP_CREATE_CANISTER]
    }

    /// Decode variant for [`UserCanisterCreateState::CreateCanister`].
    fn decode_create_canister() -> UserCanisterCreateState {
        UserCanisterCreateState::CreateCanister
    }

    /// Encode variant for [`UserCanisterCreateState::WaitForCreateCanisterSchedule`].
    fn encode_wait_for_create_canister_schedule(
        scheduled_at: &TimestampRfc3339,
        request_id: &String,
    ) -> Vec<u8> {
        let mut bytes = vec![OP_WAIT_FOR_CREATE_CANISTER_SCHEDULE];
        // write len of time
        bytes.push(scheduled_at.len() as u8);
        // write scheduled_at
        bytes.extend_from_slice(scheduled_at.as_str().as_bytes());
        // write len of request_id
        bytes.push(request_id.len() as u8);
        // write request_id
        bytes.extend_from_slice(request_id.as_bytes());

        bytes
    }

    /// Decode variant for [`UserCanisterCreateState::WaitForCreateCanisterSchedule`].
    fn decode_wait_for_create_canister_schedule(bytes: &[u8]) -> UserCanisterCreateState {
        let time_len = bytes[0] as usize;
        let time_bytes = &bytes[1..1 + time_len];
        let scheduled_at =
            TimestampRfc3339::from_utf8(time_bytes.to_vec()).expect("failed to parse time");
        let request_id_len = bytes[1 + time_len] as usize;
        let request_id_bytes = &bytes[2 + time_len..2 + time_len + request_id_len];
        let request_id =
            String::from_utf8(request_id_bytes.to_vec()).expect("failed to parse request_id");
        UserCanisterCreateState::WaitForCreateCanisterSchedule {
            scheduled_at,
            request_id,
        }
    }

    /// Encode variant for [`UserCanisterCreateState::WaitForCreateCanisterResult`].
    fn encode_wait_for_create_canister_result(request_id: &String) -> Vec<u8> {
        let mut bytes = vec![OP_WAIT_FOR_CREATE_CANISTER_RESULT];
        // write len of request_id
        bytes.push(request_id.len() as u8);
        // write request_id
        bytes.extend_from_slice(request_id.as_bytes());

        bytes
    }

    /// Decode variant for [`UserCanisterCreateState::WaitForCreateCanisterResult`].
    fn decode_wait_for_create_canister_result(bytes: &[u8]) -> UserCanisterCreateState {
        let request_id_len = bytes[0] as usize;
        let request_id_bytes = &bytes[1..1 + request_id_len];
        let request_id =
            String::from_utf8(request_id_bytes.to_vec()).expect("failed to parse request_id");
        UserCanisterCreateState::WaitForCreateCanisterResult { request_id }
    }

    /// Encode variant for [`UserCanisterCreateState::InstallCanister`].
    fn encode_install_canister(user_canister: Principal) -> Vec<u8> {
        let mut bytes = vec![OP_INSTALL_CANISTER];
        // write len of user_canister
        bytes.push(user_canister.as_slice().len() as u8);
        // write user_canister
        bytes.extend_from_slice(user_canister.as_slice());

        bytes
    }

    /// Decode variant for [`UserCanisterCreateState::InstallCanister`].
    fn decode_install_canister(bytes: &[u8]) -> UserCanisterCreateState {
        let user_canister_len = bytes[0] as usize;
        let user_canister_bytes = &bytes[1..1 + user_canister_len];
        let user_canister = Principal::from_slice(user_canister_bytes);
        UserCanisterCreateState::InstallCanister { user_canister }
    }

    /// Encode variant for [`UserCanisterCreateState::WaitForInstallCanisterSchedule`].
    fn encode_wait_for_install_canister_schedule(
        user_canister: Principal,
        scheduled_at: &TimestampRfc3339,
        request_id: &String,
    ) -> Vec<u8> {
        let mut bytes = vec![OP_WAIT_FOR_INSTALL_CANISTER_SCHEDULE];
        // write len of user_canister
        bytes.push(user_canister.as_slice().len() as u8);
        // write user_canister
        bytes.extend_from_slice(user_canister.as_slice());
        // write len of time
        bytes.push(scheduled_at.len() as u8);
        // write scheduled_at
        bytes.extend_from_slice(scheduled_at.as_str().as_bytes());
        // write len of request_id
        bytes.push(request_id.len() as u8);
        // write request_id
        bytes.extend_from_slice(request_id.as_bytes());

        bytes
    }

    /// Decode variant for [`UserCanisterCreateState::WaitForInstallCanisterSchedule`].
    fn decode_wait_for_install_canister_schedule(bytes: &[u8]) -> UserCanisterCreateState {
        let user_canister_len = bytes[0] as usize;
        let user_canister_bytes = &bytes[1..1 + user_canister_len];
        let user_canister = Principal::from_slice(user_canister_bytes);
        let time_len = bytes[1 + user_canister_len] as usize;
        let time_bytes = &bytes[2 + user_canister_len..2 + user_canister_len + time_len];
        let scheduled_at =
            TimestampRfc3339::from_utf8(time_bytes.to_vec()).expect("failed to parse time");
        let request_id_len = bytes[2 + user_canister_len + time_len] as usize;
        let request_id_bytes = &bytes
            [3 + user_canister_len + time_len..3 + user_canister_len + time_len + request_id_len];
        let request_id =
            String::from_utf8(request_id_bytes.to_vec()).expect("failed to parse request_id");
        UserCanisterCreateState::WaitForInstallCanisterSchedule {
            user_canister,
            scheduled_at,
            request_id,
        }
    }

    /// Encode variant for [`UserCanisterCreateState::WaitForInstallCanisterResult`].
    fn encode_wait_for_install_canister_result(
        user_canister: Principal,
        request_id: &String,
    ) -> Vec<u8> {
        let mut bytes = vec![OP_WAIT_FOR_INSTALL_CANISTER_RESULT];
        // write len of user_canister
        bytes.push(user_canister.as_slice().len() as u8);
        // write user_canister
        bytes.extend_from_slice(user_canister.as_slice());
        // write len of request_id
        bytes.push(request_id.len() as u8);
        // write request_id
        bytes.extend_from_slice(request_id.as_bytes());

        bytes
    }

    /// Decode variant for [`UserCanisterCreateState::WaitForInstallCanisterResult`].
    fn decode_wait_for_install_canister_result(bytes: &[u8]) -> UserCanisterCreateState {
        let user_canister_len = bytes[0] as usize;
        let user_canister_bytes = &bytes[1..1 + user_canister_len];
        let user_canister = Principal::from_slice(user_canister_bytes);
        let request_id_len = bytes[1 + user_canister_len] as usize;
        let request_id_bytes =
            &bytes[2 + user_canister_len..2 + user_canister_len + request_id_len];
        let request_id =
            String::from_utf8(request_id_bytes.to_vec()).expect("failed to parse request_id");
        UserCanisterCreateState::WaitForInstallCanisterResult {
            user_canister,
            request_id,
        }
    }

    /// Encode variant for [`UserCanisterCreateState::Completed`].
    fn encode_ok(user_canister: Principal) -> Vec<u8> {
        let mut bytes = vec![OP_OK];
        // write len of user_canister
        bytes.push(user_canister.as_slice().len() as u8);
        // write user_canister
        bytes.extend_from_slice(user_canister.as_slice());

        bytes
    }

    /// Decode variant for [`UserCanisterCreateState::Completed`].
    fn decode_ok(bytes: &[u8]) -> UserCanisterCreateState {
        println!("bytes: {:?}", bytes);
        let user_canister_len = bytes[0] as usize;
        let user_canister_bytes = &bytes[1..1 + user_canister_len];
        let user_canister = Principal::from_slice(user_canister_bytes);
        UserCanisterCreateState::Ok { user_canister }
    }

    /// Encode variant for [`UserCanisterCreateState::Failed`].
    fn encode_failed(reason: &String) -> Vec<u8> {
        let mut bytes = vec![];
        // write op code
        bytes.push(OP_FAILED);
        // write len of reason
        bytes.push(reason.len() as u8);
        // write reason
        bytes.extend_from_slice(reason.as_bytes());

        bytes
    }

    /// Decode variant for [`UserCanisterCreateState::Failed`].
    fn decode_failed(bytes: &[u8]) -> UserCanisterCreateState {
        let reason_len = bytes[0] as usize;
        let reason_bytes = &bytes[1..1 + reason_len];
        let reason = String::from_utf8(reason_bytes.to_vec()).expect("failed to parse reason");
        UserCanisterCreateState::Failed { reason }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_storable_create_canister_roundtrip() {
        let state = UserCanisterCreateState::CreateCanister;
        let bytes = state.to_bytes();
        let decoded_state = UserCanisterCreateState::from_bytes(bytes);
        assert_eq!(state, decoded_state);
    }

    #[test]
    fn test_storable_wait_for_create_canister_schedule_roundtrip() {
        let scheduled_at = TimestampRfc3339::from("2023-10-01T00:00:00Z");
        let request_id = "request_id".to_string();
        let state = UserCanisterCreateState::WaitForCreateCanisterSchedule {
            scheduled_at,
            request_id,
        };
        let bytes = state.to_bytes();
        let decoded_state = UserCanisterCreateState::from_bytes(bytes);
        assert_eq!(state, decoded_state);
    }

    #[test]
    fn test_storable_wait_for_create_canister_result_roundtrip() {
        let request_id = "request_id".to_string();
        let state = UserCanisterCreateState::WaitForCreateCanisterResult { request_id };
        let bytes = state.to_bytes();
        let decoded_state = UserCanisterCreateState::from_bytes(bytes);
        assert_eq!(state, decoded_state);
    }

    #[test]
    fn test_storable_install_canister_roundtrip() {
        let user_canister = Principal::from_slice(&[2; 29]);
        let state = UserCanisterCreateState::InstallCanister { user_canister };
        let bytes = state.to_bytes();
        let decoded_state = UserCanisterCreateState::from_bytes(bytes);
        assert_eq!(state, decoded_state);
    }

    #[test]
    fn test_storable_wait_for_install_canister_schedule_roundtrip() {
        let user_canister = Principal::from_slice(&[2; 29]);
        let scheduled_at = TimestampRfc3339::from("2023-10-01T00:00:00Z");
        let request_id = "request_id".to_string();
        let state = UserCanisterCreateState::WaitForInstallCanisterSchedule {
            user_canister,
            scheduled_at,
            request_id,
        };
        let bytes = state.to_bytes();
        let decoded_state = UserCanisterCreateState::from_bytes(bytes);
        assert_eq!(state, decoded_state);
    }

    #[test]
    fn test_storable_wait_for_install_canister_result_roundtrip() {
        let user_canister = Principal::from_slice(&[2; 29]);
        let request_id = "request_id".to_string();
        let state = UserCanisterCreateState::WaitForInstallCanisterResult {
            user_canister,
            request_id,
        };
        let bytes = state.to_bytes();
        let decoded_state = UserCanisterCreateState::from_bytes(bytes);
        assert_eq!(state, decoded_state);
    }

    #[test]
    fn test_storable_completed_roundtrip() {
        let user_canister = Principal::from_slice(&[2; 29]);
        let state = UserCanisterCreateState::Ok { user_canister };
        let bytes = state.to_bytes();
        let decoded_state = UserCanisterCreateState::from_bytes(bytes);
        assert_eq!(state, decoded_state);
    }

    #[test]
    fn test_storable_failed_roundtrip() {
        let reason = "failed".to_string();
        let state = UserCanisterCreateState::Failed { reason };
        let bytes = state.to_bytes();
        let decoded_state = UserCanisterCreateState::from_bytes(bytes);
        assert_eq!(state, decoded_state);
    }
}
