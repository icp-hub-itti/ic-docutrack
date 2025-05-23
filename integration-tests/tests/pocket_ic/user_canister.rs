use candid::Principal;
use did::orchestrator::{PublicKey, SetUserResponse};
use did::user_canister::{
    FileStatus, OwnerKey, UploadFileAtomicRequest, UploadFileContinueRequest, UploadFileRequest,
};
use integration_tests::actor::{admin, alice};
use integration_tests::{OrchestratorClient, PocketIcTestEnv, UserCanisterClient};

#[tokio::test]
async fn test_should_set_and_get_public_key() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let me = Principal::from_slice(&[1; 29]);

    let new_public_key = PublicKey::default();
    // set public key (only owner_can set it)
    client.set_public_key(new_public_key).await;
    // get public key
    let public_key = client.public_key(me).await;

    assert_eq!(new_public_key, public_key);

    env.stop().await;
}

#[tokio::test]
async fn test_should_request_file_and_get_requests() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let owner = admin();

    let request_name = "test.txt".to_string();
    let request_id = client.request_file(request_name.clone(), owner).await;
    // check randomness is working uuidv7
    // 0196f279-a899-7000-8000-000000000000
    // │        │     │    │    └───── 48 bit randomness
    // │        │     │    └────────── variant (RFC 4122)
    // │        │     └─────────────── version (7)
    // │        └───────────────────── milliseconds since Unix epoch (48 bit)
    // └────────────────────────────── (timestamp)
    assert!(!request_id.ends_with("000000000000"));

    assert_eq!(
        client.get_requests(owner).await.first().unwrap().file_name,
        request_name
    );

    env.stop().await;
}

#[tokio::test]
async fn test_should_upload_file() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let owner = admin();
    let request_name = "test.txt".to_string();
    let alias = client.request_file(request_name.clone(), owner).await;
    let alias_info = client
        .get_alias_info(alias.clone(), owner)
        .await
        .expect("alias info");

    let r = client
        .upload_file(
            UploadFileRequest {
                file_id: alias_info.file_id,
                file_content: vec![1, 2, 3],
                file_type: "txt".to_string(),
                owner_key: [1; OwnerKey::KEY_SIZE].into(),
                num_chunks: 1,
            },
            owner,
        )
        .await;
    assert!(r.is_ok());
    let public_metadata = client.get_requests(owner).await.first().unwrap().clone();

    match public_metadata.file_status {
        FileStatus::Uploaded { document_key, .. } => {
            assert_eq!(document_key, [1; OwnerKey::KEY_SIZE].into());
        }
        _ => panic!("File status is not uploaded"),
    }

    env.stop().await;
}

#[tokio::test]
async fn test_should_get_alias_info() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let owner = admin();
    let external_user = alice();
    let request_name = "test.txt".to_string();
    let alias = client.request_file(request_name.clone(), owner).await;
    let alias_info = client.get_alias_info(alias.clone(), external_user).await;

    assert_eq!(
        client
            .get_alias_info("not-an-alias".to_string(), external_user)
            .await,
        Err(did::user_canister::GetAliasInfoError::NotFound)
    );
    assert_eq!(alias_info.clone().unwrap().file_name, request_name);
    assert_eq!(alias_info.unwrap().file_id, 0);

    env.stop().await;
}

#[tokio::test]
async fn test_should_upload_file_atomic() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let owner = admin();
    let request_name = "test.txt".to_string();
    let file_id = client
        .upload_file_atomic(
            UploadFileAtomicRequest {
                name: request_name.clone(),
                content: vec![1, 2, 3],
                file_type: "txt".to_string(),
                owner_key: [1; OwnerKey::KEY_SIZE].into(),
                num_chunks: 1,
            },
            owner,
        )
        .await;
    assert_eq!(file_id, 0);
    let public_metadata = client.get_requests(owner).await.first().unwrap().clone();

    match public_metadata.file_status {
        FileStatus::Uploaded { document_key, .. } => {
            assert_eq!(document_key, [1; OwnerKey::KEY_SIZE].into());
        }
        _ => panic!("File status is not uploaded"),
    }
    assert_eq!(public_metadata.file_id, file_id);
    env.stop().await;
}

#[tokio::test]
async fn test_should_upload_file_continue() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let owner = admin();
    let request_name = "test.txt".to_string();

    let file_id = client
        .upload_file_atomic(
            UploadFileAtomicRequest {
                name: request_name.clone(),
                content: vec![1, 2, 3],
                file_type: "txt".to_string(),
                owner_key: [1; OwnerKey::KEY_SIZE].into(),
                num_chunks: 3,
            },
            owner,
        )
        .await;
    assert_eq!(file_id, 0);
    client
        .upload_file_continue(
            UploadFileContinueRequest {
                file_id,
                chunk_id: 1,
                contents: vec![4, 5, 6],
            },
            owner,
        )
        .await;

    let public_metadata = client.get_requests(owner).await.first().unwrap().clone();
    match public_metadata.file_status {
        FileStatus::PartiallyUploaded => {
            assert_eq!(public_metadata.file_id, file_id);
        }
        _ => panic!("File status is not partially uploaded"),
    }

    client
        .upload_file_continue(
            UploadFileContinueRequest {
                file_id,
                chunk_id: 2,
                contents: vec![7, 8, 9],
            },
            owner,
        )
        .await;
    let public_metadata = client.get_requests(owner).await.first().unwrap().clone();
    match public_metadata.file_status {
        FileStatus::Uploaded { document_key, .. } => {
            assert_eq!(document_key, [1; OwnerKey::KEY_SIZE].into());
        }
        _ => panic!("File status is not uploaded"),
    }

    env.stop().await;
}

#[tokio::test]
async fn test_should_download_file() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let owner = admin();
    let request_name = "test.txt".to_string();

    let file_id = client
        .upload_file_atomic(
            UploadFileAtomicRequest {
                name: request_name.clone(),
                content: vec![1, 2, 3],
                file_type: "txt".to_string(),
                owner_key: [1; OwnerKey::KEY_SIZE].into(),
                num_chunks: 3,
            },
            owner,
        )
        .await;
    assert_eq!(file_id, 0);
    client
        .upload_file_continue(
            UploadFileContinueRequest {
                file_id,
                chunk_id: 1,
                contents: vec![4, 5, 6],
            },
            owner,
        )
        .await;

    let download_response = client.download_file(file_id, 2, owner).await;
    assert_eq!(
        download_response,
        did::user_canister::FileDownloadResponse::NotUploadedFile
    );
    client
        .upload_file_continue(
            UploadFileContinueRequest {
                file_id,
                chunk_id: 2,
                contents: vec![7, 8, 9],
            },
            owner,
        )
        .await;
    let download_response = client.download_file(file_id, 2, owner).await;

    match download_response {
        did::user_canister::FileDownloadResponse::FoundFile(file_data) => {
            assert_eq!(file_data.contents, vec![7, 8, 9]);
            assert_eq!(file_data.file_type, "txt");
            assert_eq!(file_data.owner_key, [1; OwnerKey::KEY_SIZE].into());
            assert_eq!(file_data.num_chunks, 3);
        }
        _ => panic!("File not found"),
    }

    env.stop().await;
}

#[tokio::test]
async fn test_should_get_shared_files() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let orchestrator_client = OrchestratorClient::from(&env);
    let external_user = alice();
    let owner = admin();
    let request_name = "test.txt".to_string();

    // register alice on orchestrator
    let response = orchestrator_client
        .set_user(external_user, "alice".to_string(), PublicKey::default())
        .await;
    assert_eq!(response, SetUserResponse::Ok);

    let file_id = client
        .upload_file_atomic(
            UploadFileAtomicRequest {
                name: request_name.clone(),
                content: vec![1, 2, 3],
                file_type: "txt".to_string(),
                owner_key: [1; OwnerKey::KEY_SIZE].into(),
                num_chunks: 1,
            },
            owner,
        )
        .await;

    let shared_files = client.get_shared_files(owner, external_user).await;
    assert_eq!(shared_files.len(), 0);

    // share file with alice
    assert_eq!(
        client
            .share_file(
                owner,
                file_id,
                external_user,
                [1; OwnerKey::KEY_SIZE].into()
            )
            .await,
        did::user_canister::FileSharingResponse::Ok
    );
    let shared_files = client.get_shared_files(owner, external_user).await;
    assert_eq!(shared_files.len(), 1);
    assert_eq!(shared_files[0].file_id, file_id);

    env.stop().await;
}

#[tokio::test]
async fn test_should_delete_file() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let owner = admin();
    let request_name = "test.txt".to_string();

    let file_id = client
        .upload_file_atomic(
            UploadFileAtomicRequest {
                name: request_name.clone(),
                content: vec![1, 2, 3],
                file_type: "txt".to_string(),
                owner_key: [1; OwnerKey::KEY_SIZE].into(),
                num_chunks: 1,
            },
            owner,
        )
        .await;
    assert_eq!(file_id, 0);
    client
        .delete_file(owner, file_id)
        .await
        .expect("delete file");

    assert_eq!(
        client.get_requests(owner).await.len(),
        0,
        "file should be deleted"
    );

    env.stop().await;
}

#[tokio::test]
async fn test_should_delete_shared_file() {
    let env = PocketIcTestEnv::init().await;
    let client = UserCanisterClient::from(&env);
    let orchestrator_client = OrchestratorClient::from(&env);
    let external_user = alice();
    let owner = admin();
    let request_name = "test.txt".to_string();

    // register alice on orchestrator
    let response = orchestrator_client
        .set_user(external_user, "alice".to_string(), PublicKey::default())
        .await;
    assert_eq!(response, SetUserResponse::Ok);

    let file_id = client
        .upload_file_atomic(
            UploadFileAtomicRequest {
                name: request_name.clone(),
                content: vec![1, 2, 3],
                file_type: "txt".to_string(),
                owner_key: [1; OwnerKey::KEY_SIZE].into(),
                num_chunks: 1,
            },
            owner,
        )
        .await;

    // share file with alice
    assert_eq!(
        client
            .share_file(
                owner,
                file_id,
                external_user,
                [1; OwnerKey::KEY_SIZE].into()
            )
            .await,
        did::user_canister::FileSharingResponse::Ok
    );

    // delete shared file
    client
        .delete_file(owner, file_id)
        .await
        .expect("delete file");

    assert_eq!(
        client.get_requests(owner).await.len(),
        0,
        "file should be deleted"
    );

    env.stop().await;
}
