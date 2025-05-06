mod orchestrator;
use integration_tests::PocketIcTestEnv;

#[tokio::test]
async fn test_should_setup_test_env() {
    PocketIcTestEnv::init().await;
}
