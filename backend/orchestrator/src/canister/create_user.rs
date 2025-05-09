use std::time::Duration;

use candid::Principal;
use did::orbit_station::{RequestOperation, RequestStatus, TimestampRfc3339};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::client::OrbitStationClient;
use crate::storage::config::Config;
use crate::storage::user_canister::{UserCanisterCreateState, UserCanisterStorage};
use crate::utils::{datetime, trap};

/// Default interval between each operation.
const DEFAULT_INTERVAL: Duration = Duration::from_secs(10);
/// Interval to wait for the Orbit Station to process the request.
const ORBIT_STATION_REQUEST_INTERVAL: Duration = Duration::from_secs(60);
/// The WASM file for the user canister.
const USER_CANISTER_WASM: &[u8] = include_bytes!("../../../../.artifact/backend.wasm.gz");

/// A service to create the user canister for a user.
#[derive(Debug, Clone, Copy)]
pub struct CreateUserStateMachine {
    orbit_station: Principal,
    user: Principal,
}

impl CreateUserStateMachine {
    /// Creates a new instance of [`CreateUserStateMachine`] and starts it.
    pub fn start(orbit_station: Principal, user: Principal) {
        let state_machine = Self {
            orbit_station,
            user,
        };

        // Initialize the user canister creation state.
        UserCanisterStorage::init_create_state(state_machine.user);

        state_machine.tick(Duration::from_secs(1));
    }

    /// Set a timer to wait for the specified duration and then run the state machine.
    fn tick(self, delay: Duration) {
        // run state machine
        ic_cdk_timers::set_timer(delay, move || {
            ic_cdk::futures::spawn(async move {
                self.run().await;
            });
        });
    }

    /// Run a step of the state machine.
    async fn run(self) {
        // load state from storage
        let current_state = UserCanisterStorage::get_create_state(self.user)
            .unwrap_or_else(|| trap("User canister creation state not found"));

        let current_state_id = std::mem::discriminant(&current_state);

        let new_state = match current_state {
            UserCanisterCreateState::CreateCanister => self.create_canister().await,
            UserCanisterCreateState::WaitForCreateCanisterSchedule { request_id, .. } => {
                self.check_create_canister_result(request_id).await
            }
            UserCanisterCreateState::WaitForCreateCanisterResult { request_id, .. } => {
                self.check_create_canister_result(request_id).await
            }
            UserCanisterCreateState::InstallCanister { user_canister } => {
                self.install_canister(user_canister).await
            }
            UserCanisterCreateState::WaitForInstallCanisterSchedule {
                request_id,
                user_canister,
                ..
            } => {
                self.check_install_canister_result(request_id, user_canister)
                    .await
            }
            UserCanisterCreateState::WaitForInstallCanisterResult {
                request_id,
                user_canister,
            } => {
                self.check_install_canister_result(request_id, user_canister)
                    .await
            }
            UserCanisterCreateState::Ok { user_canister } => {
                self.complete(user_canister);

                return; // stop the state machine
            }
            UserCanisterCreateState::Failed { .. } => return, // stop the state machine
        };

        // update state in storage if the variant type has changed
        if std::mem::discriminant(&new_state) != current_state_id {
            UserCanisterStorage::set_create_state(self.user, new_state.clone());
        }

        // schedule next step
        let delay = Self::delay(&new_state);
        self.tick(delay);
    }

    /// Sends a request to the Orbit Station canister to install the user canister.
    async fn create_canister(&self) -> UserCanisterCreateState {
        let orbit_station_admin = Config::get_orbit_station_admin();

        match OrbitStationClient::from(self.orbit_station)
            .create_user_canister(self.user, orbit_station_admin)
            .await
        {
            Ok(Ok(request)) => UserCanisterCreateState::WaitForCreateCanisterResult {
                request_id: request.request.id,
            },
            Ok(Err(e)) => UserCanisterCreateState::Failed {
                reason: format!("failed to create canister: {e:?}"),
            },
            Err(err) => UserCanisterCreateState::Failed {
                reason: format!("failed to create canister: {err}"),
            },
        }
    }

    /// Checks the result of the create canister request.
    async fn check_create_canister_result(&self, request_id: String) -> UserCanisterCreateState {
        // send request to get the request status
        let response = match OrbitStationClient::from(self.orbit_station)
            .get_request_status(request_id.clone())
            .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                return UserCanisterCreateState::Failed {
                    reason: format!(
                        "failed to get request status for {request_id} (create_canister): {e:?}"
                    ),
                };
            }
            Err(err) => {
                return UserCanisterCreateState::Failed {
                    reason: format!(
                        "failed to get request status for {request_id} (create_canister): {err}"
                    ),
                };
            }
        };

        let status = response.request.status;
        match status {
            RequestStatus::Completed { .. } => {
                // operation is successful; get canister id
                let op = match response.request.operation {
                    RequestOperation::CreateExternalCanister(op) => op,
                    _ => trap(format!(
                        "unexpected operation type: {:?}",
                        response.request.operation
                    )),
                };

                match op.canister_id {
                    Some(user_canister) => {
                        UserCanisterCreateState::InstallCanister { user_canister }
                    }
                    None => UserCanisterCreateState::Failed {
                        reason: "Canister ID not found after creation".to_string(),
                    },
                }
            }
            RequestStatus::Failed { reason } => UserCanisterCreateState::Failed {
                reason: format!("failed to create canister: {}", reason.unwrap_or_default()),
            },
            RequestStatus::Rejected => UserCanisterCreateState::Failed {
                reason: "create_canister request rejected".to_string(),
            },
            RequestStatus::Cancelled { reason } => UserCanisterCreateState::Failed {
                reason: format!(
                    "create_canister request cancelled: {}",
                    reason.unwrap_or_default()
                ),
            },
            RequestStatus::Scheduled { scheduled_at } => {
                // operation is scheduled; update state
                UserCanisterCreateState::WaitForCreateCanisterSchedule {
                    request_id,
                    scheduled_at,
                }
            }
            RequestStatus::Approved | RequestStatus::Created | RequestStatus::Processing { .. } => {
                // operation is in progress; update state
                UserCanisterCreateState::WaitForCreateCanisterResult { request_id }
            }
        }
    }

    /// Installs the user canister by sending a request to the Orbit Station canister.
    async fn install_canister(&self, user_canister: Principal) -> UserCanisterCreateState {
        match OrbitStationClient::from(self.orbit_station)
            .install_user_canister(user_canister, self.user, USER_CANISTER_WASM)
            .await
        {
            Ok(Ok(request)) => UserCanisterCreateState::WaitForInstallCanisterResult {
                request_id: request.request.id,
                user_canister,
            },
            Ok(Err(e)) => UserCanisterCreateState::Failed {
                reason: format!("failed to install canister: {e:?}"),
            },
            Err(err) => UserCanisterCreateState::Failed {
                reason: format!("failed to install canister: {err}"),
            },
        }
    }

    /// Checks the result of the install canister request.
    async fn check_install_canister_result(
        &self,
        request_id: String,
        user_canister: Principal,
    ) -> UserCanisterCreateState {
        // send request to get the request status
        let response = match OrbitStationClient::from(self.orbit_station)
            .get_request_status(request_id.clone())
            .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                return UserCanisterCreateState::Failed {
                    reason: format!(
                        "failed to get request status for {request_id} (install_canister): {e:?}"
                    ),
                };
            }
            Err(err) => {
                return UserCanisterCreateState::Failed {
                    reason: format!(
                        "failed to get request status for {request_id} (install_canister): {err}"
                    ),
                };
            }
        };

        let status = response.request.status;
        match status {
            RequestStatus::Completed { .. } => UserCanisterCreateState::Ok { user_canister },
            RequestStatus::Failed { reason } => UserCanisterCreateState::Failed {
                reason: format!("failed to install canister: {}", reason.unwrap_or_default()),
            },
            RequestStatus::Rejected => UserCanisterCreateState::Failed {
                reason: "install_canister request rejected".to_string(),
            },
            RequestStatus::Cancelled { reason } => UserCanisterCreateState::Failed {
                reason: format!(
                    "install_canister request cancelled: {}",
                    reason.unwrap_or_default()
                ),
            },
            RequestStatus::Scheduled { scheduled_at } => {
                // operation is scheduled; update state
                UserCanisterCreateState::WaitForInstallCanisterSchedule {
                    request_id,
                    scheduled_at,
                    user_canister,
                }
            }
            RequestStatus::Approved | RequestStatus::Created | RequestStatus::Processing { .. } => {
                // operation is in progress; update state
                UserCanisterCreateState::WaitForInstallCanisterResult {
                    request_id,
                    user_canister,
                }
            }
        }
    }

    /// Complete user canister creation by setting the user canister ID in storage.
    fn complete(&self, user_canister: Principal) {
        UserCanisterStorage::set_user_canister(self.user, user_canister);
    }

    /// Get interval to sleep for the next operation [`UserCanisterCreateState`].
    /// Returns [`Duration`].
    fn delay(op: &UserCanisterCreateState) -> Duration {
        match op {
            UserCanisterCreateState::CreateCanister => DEFAULT_INTERVAL,
            UserCanisterCreateState::WaitForCreateCanisterSchedule { scheduled_at, .. } => {
                Self::scheduled_at_time_diff(datetime(), scheduled_at)
            }
            UserCanisterCreateState::WaitForCreateCanisterResult { .. } => {
                ORBIT_STATION_REQUEST_INTERVAL
            }
            UserCanisterCreateState::InstallCanister { .. } => DEFAULT_INTERVAL,
            UserCanisterCreateState::WaitForInstallCanisterSchedule { scheduled_at, .. } => {
                Self::scheduled_at_time_diff(datetime(), scheduled_at)
            }
            UserCanisterCreateState::WaitForInstallCanisterResult { .. } => {
                ORBIT_STATION_REQUEST_INTERVAL
            }
            UserCanisterCreateState::Ok { .. } => Duration::ZERO,
            UserCanisterCreateState::Failed { .. } => Duration::ZERO,
        }
    }

    /// Get the time difference between the current time and the scheduled time.
    ///
    /// If the scheduled time is in the past, return [`DEFAULT_INTERVAL`].
    fn scheduled_at_time_diff(date: OffsetDateTime, scheduled_at: &TimestampRfc3339) -> Duration {
        let scheduled_at = OffsetDateTime::parse(scheduled_at, &Rfc3339)
            .unwrap_or_else(|_| trap("Failed to parse scheduled_at"));

        (scheduled_at.unix_timestamp() as u64)
            .checked_sub(date.unix_timestamp() as u64)
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_INTERVAL)
    }
}

#[cfg(test)]
mod test {

    use time::{Date, Time};

    use super::*;

    #[test]
    fn test_should_return_schedule_diff_in_the_future() {
        let test_date = OffsetDateTime::new_utc(
            Date::from_calendar_date(2021, time::Month::May, 6).unwrap(),
            Time::from_hms(19, 10, 8).unwrap(),
        );

        let scheduled_at = "2021-05-06T19:17:10.000000031Z".to_string();

        let diff = CreateUserStateMachine::scheduled_at_time_diff(test_date, &scheduled_at);

        // diff is 7 minutes and 2 seconds
        assert_eq!(diff.as_secs(), 7 * 60 + 2);
    }

    #[test]
    fn test_should_return_schedule_diff_if_in_the_past() {
        let test_date = OffsetDateTime::new_utc(
            Date::from_calendar_date(2024, time::Month::May, 6).unwrap(),
            Time::from_hms(19, 10, 8).unwrap(),
        );

        let scheduled_at = "2021-05-06T19:17:10.000000031Z".to_string();

        let diff = CreateUserStateMachine::scheduled_at_time_diff(test_date, &scheduled_at);

        assert_eq!(diff, DEFAULT_INTERVAL);
    }
}
