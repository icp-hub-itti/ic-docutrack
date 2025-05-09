use candid::Principal;
use time::OffsetDateTime;

/// Utility functions to trap the canister.
///
/// The reason of this is that you cannot use [`panic!`] on canisters and you can't use
/// [`ic_cdk::trap`] in test units.
pub fn trap<S>(msg: S) -> !
where
    S: AsRef<str>,
{
    if cfg!(target_family = "wasm") {
        ic_cdk::trap(msg)
    } else {
        panic!("{}", msg.as_ref())
    }
}

/// Returns the caller of a message as [`Principal`].
///
/// The reason of this is that you cannot use [`ic_cdk::api::msg_caller`] on test units.
pub fn msg_caller() -> Principal {
    if cfg!(target_family = "wasm") {
        ic_cdk::api::msg_caller()
    } else {
        Principal::from_slice(&[1; 29])
    }
}

/// Returns current time in nanoseconds
pub fn time() -> u64 {
    if cfg!(target_family = "wasm") {
        ic_cdk::api::time()
    } else {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime before UNIX EPOCH");
        time.as_nanos() as u64
    }
}

/// Returns current datetime
pub fn datetime() -> OffsetDateTime {
    let time = time();

    OffsetDateTime::from_unix_timestamp_nanos(time as i128)
        .unwrap_or_else(|_| trap("Failed to convert time to OffsetDateTime"))
}
