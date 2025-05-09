use candid::Principal;
use did::orchestrator::{
    GetUsersResponse, PUBKEY_SIZE, PublicUser, SetUserResponse, WhoamiResponse,
};
use integration_tests::{OrchestratorClient, PocketIcTestEnv, TestEnv};

#[tokio::test]
async fn test_should_get_orbit_station() {
    let env = PocketIcTestEnv::init().await;
    let orbit_station = OrchestratorClient::from(&env).orchestrator_client().await;

    assert_eq!(orbit_station, env.orbit_station());

    env.stop().await;
}

#[tokio::test]
async fn test_should_register_user() {
    let env = PocketIcTestEnv::init().await;
    let client = OrchestratorClient::from(&env);

    let me = Principal::from_slice(&[1; 29]);

    let username = "foo".to_string();
    let public_key = [1; PUBKEY_SIZE];

    // we check if username is available
    assert!(!client.username_exists(username.clone()).await,);

    // register
    let response = client.set_user(me, username.clone(), public_key).await;
    assert_eq!(response, SetUserResponse::Ok);

    // check if username exists
    assert!(client.username_exists(username.clone()).await);

    // who am i
    let whoami = client.who_am_i(me).await;
    assert_eq!(
        whoami,
        WhoamiResponse::KnownUser(PublicUser {
            username,
            public_key,
            ic_principal: me,
        })
    );

    env.stop().await;
}

#[tokio::test]
async fn test_should_not_register_user_if_anonymous() {
    let env = PocketIcTestEnv::init().await;
    let client = OrchestratorClient::from(&env);

    let username = "foo".to_string();
    let public_key = [1; PUBKEY_SIZE];
    let response = client
        .set_user(Principal::anonymous(), username, public_key)
        .await;
    assert_eq!(response, SetUserResponse::AnonymousCaller);

    env.stop().await;
}

#[tokio::test]
async fn test_should_not_get_users_if_anonymous() {
    let env = PocketIcTestEnv::init().await;
    let client = OrchestratorClient::from(&env);

    let users = client.get_users(Principal::anonymous()).await;
    assert_eq!(users, GetUsersResponse::PermissionError);

    env.stop().await;
}

#[tokio::test]
async fn test_should_create_user_canister() {
    let env = PocketIcTestEnv::init().await;
    let client = OrchestratorClient::from(&env);

    let me = Principal::from_slice(&[1; 29]);
    let username = "foo".to_string();
    let public_key = [1; PUBKEY_SIZE];

    // create user canister
    let response = client.set_user(me, username, public_key).await;
    assert_eq!(response, SetUserResponse::Ok);

    // wait for user canister to be created
    let user_canister = client.wait_for_user_canister(me).await;
    assert_ne!(user_canister, Principal::anonymous());

    env.stop().await;
}
