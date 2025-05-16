use candid::Principal;
use did::orbit_station::{
    Allow, AuthScope, CanisterInstallMode, ChangeExternalCanisterOperationInput,
    CreateExternalCanisterOperationInput, CreateExternalCanisterOperationKind,
    CreateExternalCanisterOperationKindCreateNew, CreateRequestInput, CreateRequestResult,
    ExternalCanisterCallPermission, ExternalCanisterMetadata, ExternalCanisterPermissions,
    ExternalCanisterRequestPoliciesCreateInput, GetRequestInput, GetRequestResult,
    RequestExecutionSchedule, RequestOperationInput, ValidationMethodResourceTarget,
};
use did::user_canister::UserCanisterInstallArgs;
use ic_cdk::call::{Call, CallRejected, CallResult, Error as CallError};

/// Client for the Orbit Station canister.
pub struct OrbitStationClient {
    principal: Principal,
}

impl From<Principal> for OrbitStationClient {
    fn from(principal: Principal) -> Self {
        OrbitStationClient { principal }
    }
}

impl OrbitStationClient {
    /// Send a request to the Orbit Station canister to get the status of a request by its ID.
    ///
    /// Returns [`GetRequestResult`] containing the status of the request.
    pub async fn get_request_status(&self, request_id: String) -> CallResult<GetRequestResult> {
        let request = GetRequestInput {
            request_id,
            with_full_info: None,
        };

        Call::unbounded_wait(self.principal, "get_request")
            .with_arg(request)
            .await?
            .candid()
            .map_err(CallError::from)
    }

    /// Send a request to the Orbit Station canister to create the user canister.
    pub async fn create_user_canister(
        &self,
        user: Principal,
        admin: String,
    ) -> CallResult<CreateRequestResult> {
        let request = CreateRequestInput {
            title: Some(format!("create user canister for user {user}")),
            summary: None,
            execution_plan: Some(RequestExecutionSchedule::Immediate),
            expiration_dt: None,
            operation: RequestOperationInput::CreateExternalCanister(
                CreateExternalCanisterOperationInput {
                    permissions: ExternalCanisterPermissions {
                        calls: vec![ExternalCanisterCallPermission {
                            execution_method: "set_state".to_string(),
                            allow: Allow {
                                users: vec![admin.clone()],
                                user_groups: vec![],
                                auth_scope: AuthScope::Authenticated,
                            },
                            validation_method: ValidationMethodResourceTarget::No,
                        }],
                        read: Allow {
                            users: vec![admin.clone()],
                            user_groups: vec![],
                            auth_scope: AuthScope::Authenticated,
                        },
                        change: Allow {
                            users: vec![admin.clone()],
                            user_groups: vec![],
                            auth_scope: AuthScope::Authenticated,
                        },
                    },
                    metadata: Some(vec![
                        ExternalCanisterMetadata {
                            key: "name".to_string(),
                            value: "user canister".to_string(),
                        },
                        ExternalCanisterMetadata {
                            key: "owner".to_string(),
                            value: user.to_text(),
                        },
                    ]),
                    kind: CreateExternalCanisterOperationKind::CreateNew(
                        CreateExternalCanisterOperationKindCreateNew {
                            initial_cycles: Some(2_000_000_000_000),
                            subnet_selection: None,
                        },
                    ),
                    name: format!("user_canister_{user}"), // name must be unique
                    labels: None,
                    description: Some(format!("ic-docutrack user canister for {user}")),
                    request_policies: ExternalCanisterRequestPoliciesCreateInput {
                        calls: vec![],
                        change: vec![],
                    },
                },
            ),
        };

        Call::unbounded_wait(self.principal, "create_request")
            .with_arg(request)
            .await?
            .candid()
            .map_err(CallError::from)
    }

    /// Install user canister with the given `wasm` module.
    pub async fn install_user_canister(
        &self,
        canister_id: Principal,
        owner: Principal,
        wasm: &[u8],
        arg: UserCanisterInstallArgs,
    ) -> CallResult<CreateRequestResult> {
        let arg = candid::encode_one(arg).map_err(|e| {
            CallError::CallRejected(CallRejected::with_rejection(
                1,
                format!("Failed to encode init arg: {}", e),
            ))
        })?;

        let request = CreateRequestInput {
            title: Some(format!("install user canister for user {owner}")),
            summary: Some(format!(
                "install user canister {canister_id} for user {owner}"
            )),
            execution_plan: Some(RequestExecutionSchedule::Immediate),
            expiration_dt: None,
            operation: RequestOperationInput::ChangeExternalCanister(
                ChangeExternalCanisterOperationInput {
                    canister_id,
                    arg: Some(arg.into()),
                    module_extra_chunks: None,
                    mode: CanisterInstallMode::Install,
                    module: wasm.to_vec().into(),
                },
            ),
        };

        Call::unbounded_wait(self.principal, "create_request")
            .with_arg(request)
            .await?
            .candid()
            .map_err(CallError::from)
    }
}
