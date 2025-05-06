use candid::Encode;
use did::State;
// use did::orchestrator::SetUserResponse;
use orchestrator::SetUserResponse;
use integration_tests::{PocketIcTestEnv, TestEnv as _};
use integration_tests::actor::bob;

#[tokio::test]
async fn test_should_set_and_get_user() {
    let env = PocketIcTestEnv::init().await;
    let user_canister = env.user_canister1();
    let orchestrator = env.orchestrator();

    // Set user info
    let set_user_info_payload = Encode! {
        &"bob".to_string(),
        &vec![1, 2, 3],
        &user_canister
    }
    .unwrap();

    let res = env.update::<State>(
        orchestrator,
        bob(),
        "set_user",
        set_user_info_payload,
    )
    .await
    .unwrap();

    assert_eq!(res,SetUserResponse::Ok);

}