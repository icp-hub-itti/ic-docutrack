mod share;

use std::collections::BTreeMap;

use candid::Principal;
use did::orchestrator::ShareFileResponse;
use did::user_canister::{
    AliasInfo, DeleteFileResponse, FileData, FileDownloadResponse, FileSharingResponse, FileStatus,
    GetAliasInfoError, OwnerKey, PublicFileMetadata, UploadFileAtomicRequest,
    UploadFileContinueRequest, UploadFileContinueResponse, UploadFileError,
    UserCanisterInstallArgs,
};
use did::utils::trap;

use crate::aliases::{AliasGenerator, Randomness};
use crate::client::OrchestratorClient;
use crate::storage::config::Config;
use crate::storage::files::{
    File, FileAliasIndexStorage, FileContent, FileContentsStorage, FileCountStorage,
    FileDataStorage, FileId, FileMetadata, FileSharesStorage, OwnedFilesStorage, UploadedChunks,
};
use crate::utils::time;

/// API for the backend canister
pub struct Canister;

impl Canister {
    /// Initialize the canister with the given arguments.
    pub fn init(args: UserCanisterInstallArgs) {
        let UserCanisterInstallArgs::Init(args) = args else {
            trap("Invalid arguments");
        };

        Config::set_orchestrator(args.orchestrator);
        Config::set_owner(args.owner);
    }

    /// Request a file
    pub async fn request_file<S: Into<String>>(caller: Principal, request_name: S) -> String {
        if caller != Config::get_owner() {
            trap("Only the owner can request a file");
        }
        let randomness = Randomness::new().await;

        // generate a file ID and alias
        let file_id = FileCountStorage::generate_file_id();
        let alias = AliasGenerator::new(randomness).generate_uuidv7();

        // make the file
        let file = File {
            metadata: FileMetadata {
                file_name: request_name.into(),
                user_public_key: Config::get_owner_public_key(),
                requester_principal: caller,
                requested_at: time(),
                uploaded_at: None,
            },
            content: FileContent::Pending {
                alias: alias.clone(),
            },
        };

        // associate the file ID with its data
        FileDataStorage::set_file(&file_id, file);
        // associate the alias with the file ID
        FileAliasIndexStorage::set_file_id(&alias, &file_id);
        // associate
        OwnedFilesStorage::add_owned_file(&file_id);

        alias
    }

    /// Get active requests for the caller
    ///
    // FIXME: maybe rename this function or see in what context is used
    // FIXME: maybe more suitable name is get_owned_files ??
    pub fn get_requests(caller: Principal) -> Vec<PublicFileMetadata> {
        if caller != Config::get_owner() {
            trap("Only the owner can get requests for a file");
        }
        OwnedFilesStorage::get_owned_files()
            .iter()
            .map(|file_id| PublicFileMetadata {
                file_id: *file_id,
                file_name: FileDataStorage::get_file(file_id)
                    .expect("file must exist")
                    .metadata
                    .file_name
                    .clone(),
                shared_with: Self::get_allowed_users(caller, file_id),
                file_status: Self::get_file_status(file_id),
            })
            .collect()
    }

    /// upload a file with the given [`FileId`] and file content.
    ///
    /// to be triggered by requested file uploads
    pub fn upload_file(
        file_id: FileId,
        file_content: Vec<u8>,
        file_type: String,
        owner_key: OwnerKey,
        num_chunks: u64,
    ) -> Result<(), UploadFileError> {
        let file = FileDataStorage::get_file(&file_id);
        if file.is_none() {
            return Err(UploadFileError::NotRequested);
        }
        let Some(mut file) = file else {
            return Err(UploadFileError::NotRequested);
        };
        let shared_keys = BTreeMap::new();
        let chunk_id = 0;

        let alias = match &file.content {
            FileContent::Pending { alias } => {
                let alias = alias.clone();
                if num_chunks == 1 {
                    file.content = FileContent::Uploaded {
                        file_type,
                        owner_key,
                        shared_keys,
                        num_chunks,
                    };
                } else {
                    let mut uploaded_chunks = UploadedChunks::default();
                    uploaded_chunks.insert(chunk_id);
                    file.content = FileContent::PartiallyUploaded {
                        num_chunks,
                        uploaded_chunks,
                        file_type,
                        owner_key,
                        shared_keys,
                    };
                }
                file.metadata.uploaded_at = Some(time());
                //persist file
                FileDataStorage::set_file(&file_id, file);

                //add file to the storage
                FileContentsStorage::set_file_contents(&file_id, &chunk_id, file_content);
                alias
            }
            FileContent::Uploaded { .. } | FileContent::PartiallyUploaded { .. } => {
                return Err(UploadFileError::AlreadyUploaded);
            }
        };

        // removing alias from the index
        FileAliasIndexStorage::remove_file_id(&alias);

        Ok(())
    }

    /// Upload file Atomic
    /// to be triggered by owners, no need to request file
    pub fn upload_file_atomic(caller: Principal, request: UploadFileAtomicRequest) -> FileId {
        if caller != Config::get_owner() {
            trap("Only the owner can upload a file");
        }
        let file_id = FileCountStorage::generate_file_id();
        let chunk_id = 0;
        let content = if request.num_chunks == 1 {
            FileContent::Uploaded {
                file_type: request.file_type,
                owner_key: request.owner_key,
                shared_keys: BTreeMap::new(),
                num_chunks: request.num_chunks,
            }
        } else {
            let mut uploaded_chunks = UploadedChunks::default();
            uploaded_chunks.insert(chunk_id);

            FileContent::PartiallyUploaded {
                num_chunks: request.num_chunks,
                uploaded_chunks,
                file_type: request.file_type,
                owner_key: request.owner_key,
                shared_keys: BTreeMap::new(),
            }
        };

        // Aff File to content storage
        FileContentsStorage::set_file_contents(&file_id, &chunk_id, request.content);
        // Add file to the file storage
        let file = File {
            metadata: FileMetadata {
                file_name: request.name,
                user_public_key: Config::get_owner_public_key(),
                requester_principal: caller,
                requested_at: time(),
                uploaded_at: Some(time()),
            },
            content,
        };
        FileDataStorage::set_file(&file_id, file);

        OwnedFilesStorage::add_owned_file(&file_id);

        file_id
    }

    /// Upload file continue
    pub fn upload_file_continue(request: UploadFileContinueRequest) -> UploadFileContinueResponse {
        let Some(mut file) = FileDataStorage::get_file(&request.file_id) else {
            return UploadFileContinueResponse::FileNotFound;
        };

        let chunk_id = request.chunk_id;

        // Update file content
        match &file.content {
            FileContent::Uploaded { .. } => {
                return UploadFileContinueResponse::FileAlreadyUploaded;
            }
            FileContent::PartiallyUploaded {
                num_chunks,
                uploaded_chunks,
                file_type,
                owner_key,
                shared_keys,
            } => {
                // Check if the chunk is already uploaded
                if uploaded_chunks.contains(&chunk_id) {
                    return UploadFileContinueResponse::ChunkAlreadyUploaded;
                }
                // Check if the chunk ID is valid
                if chunk_id >= *num_chunks {
                    return UploadFileContinueResponse::ChunkOutOfBounds;
                }
                // Add the chunk to the uploaded chunks
                let mut uploaded_chunks = uploaded_chunks.clone();
                uploaded_chunks.insert(chunk_id);
                // Check if all chunks are uploaded
                if uploaded_chunks.len() == *num_chunks as usize {
                    file.content = FileContent::Uploaded {
                        file_type: file_type.clone(),
                        owner_key: *owner_key,
                        shared_keys: shared_keys.clone(),
                        num_chunks: *num_chunks,
                    };
                } else {
                    // If not all chunks are uploaded, update the file content
                    file.content = FileContent::PartiallyUploaded {
                        num_chunks: *num_chunks,
                        uploaded_chunks: uploaded_chunks.clone(),
                        file_type: file_type.clone(),
                        owner_key: *owner_key,
                        shared_keys: shared_keys.clone(),
                    };
                }
            }
            _ => {}
        }
        // Add file to the content storage
        FileContentsStorage::set_file_contents(&request.file_id, &chunk_id, request.contents);

        // Persist file
        FileDataStorage::set_file(&request.file_id, file);

        UploadFileContinueResponse::Ok
    }

    /// Share file with user
    pub async fn share_file(
        caller: Principal,
        user_id: Principal,
        file_id: FileId,
        file_key_encrypted_for_user: OwnerKey,
    ) -> FileSharingResponse {
        if caller != Config::get_owner() {
            trap("Only the owner can share a file");
        }

        // check whether we can share the file
        match share::CanisterShareFile::check_shareable(file_id) {
            FileSharingResponse::Ok => {}
            err => {
                return err;
            }
        }

        // Index Share file on the orchestrator
        if cfg!(target_family = "wasm") {
            match OrchestratorClient::from(Config::get_orchestrator())
                .share_file(user_id, file_id)
                .await
            {
                Err(err) => {
                    trap(
                        format!("Error indexing sharing file on orchestrator: {:?}", err).as_str(),
                    );
                }
                Ok(ShareFileResponse::Ok) => {}
                Ok(share_err) => {
                    trap(format!("Error sharing file on orchestrator: {:?}", share_err).as_str());
                }
            }
        }

        share::CanisterShareFile::share_file(user_id, file_id, file_key_encrypted_for_user)
    }

    /// Share file with users
    pub async fn share_file_with_users(
        caller: Principal,
        users: Vec<Principal>,
        file_id: FileId,
        file_key_encrypted_for_user: Vec<OwnerKey>,
    ) {
        if caller != Config::get_owner() {
            trap("Only the owner can share a file");
        }

        // check whether we can share the file
        match share::CanisterShareFile::check_shareable(file_id) {
            FileSharingResponse::Ok => {}
            err => {
                trap(format!("Error sharing file: {:?}", err).as_str());
            }
        }

        // Index files on the orchestrator
        if cfg!(target_family = "wasm") {
            match OrchestratorClient::from(Config::get_orchestrator())
                .share_file_with_users(&users, file_id)
                .await
            {
                Err(err) => {
                    trap(
                        format!("Error indexing sharing file on orchestrator: {:?}", err).as_str(),
                    );
                }
                Ok(ShareFileResponse::Ok) => {}
                Ok(share_err) => {
                    trap(format!("Error sharing file on orchestrator: {:?}", share_err).as_str());
                }
            }
        }

        // commit changes to the canister storage
        for (user, decryption_key) in users.iter().zip(file_key_encrypted_for_user.iter()) {
            match share::CanisterShareFile::share_file(*user, file_id, *decryption_key) {
                FileSharingResponse::Ok => {}
                err => {
                    trap(format!("Error sharing file: {:?}", err).as_str());
                }
            }
        }
    }

    /// Revoke file sharing
    pub async fn revoke_file_sharing(caller: Principal, user_id: Principal, file_id: FileId) {
        if caller != Config::get_owner() {
            trap("Only the owner can revoke file sharing");
        }

        // get file first checking if it exists
        let Some(mut file) = FileDataStorage::get_file(&file_id) else {
            trap("File not found");
        };

        // first call the orchestrator to revoke the file, since this can fail
        if cfg!(target_family = "wasm") {
            // Revoke files on the orchestrator
            if let Err(err) = OrchestratorClient::from(Config::get_orchestrator())
                .revoke_share_file(user_id, file_id)
                .await
            {
                trap(format!("Error revoking shared file on orchestrator: {:?}", err).as_str());
            }
        }

        // remove user from file shares (cannot fail)
        match &mut file.content {
            FileContent::Uploaded { shared_keys, .. }
            | FileContent::PartiallyUploaded { shared_keys, .. } => {
                shared_keys.remove(&user_id);
            }
            _ => {}
        }

        // persist file (cannot fail)
        FileDataStorage::set_file(&file_id, file);

        // remove file from user shares (cannot fail)
        FileSharesStorage::revoke(&user_id, &file_id);
    }

    /// Download file
    pub fn download_file(
        caller: Principal,
        file_id: FileId,
        chunk_id: u64,
    ) -> FileDownloadResponse {
        let file = FileDataStorage::get_file(&file_id);
        if file.is_none() {
            return FileDownloadResponse::NotFoundFile;
        }
        let file = file.unwrap();
        // Check if the file is shared with the caller or if the caller is the owner
        let file_c = match &file.content {
            FileContent::Pending { .. } | FileContent::PartiallyUploaded { .. } => {
                return FileDownloadResponse::NotUploadedFile;
            }
            FileContent::Uploaded {
                shared_keys,
                num_chunks,
                file_type,
                owner_key,
            } => {
                if !shared_keys.contains_key(&caller) && caller != file.metadata.requester_principal
                {
                    return FileDownloadResponse::PermissionError;
                }
                let num_chunks = *num_chunks;
                let file_type = file_type.clone();
                // if the caller is the owner, use the owner key
                // else use the shared key
                let owner_key = match caller == file.metadata.requester_principal {
                    true => *owner_key,
                    false => *shared_keys.get(&caller).unwrap(),
                };

                (num_chunks, file_type, owner_key)
            }
        };
        let contents = FileContentsStorage::get_file_contents(&file_id, &chunk_id);
        if contents.is_none() {
            return FileDownloadResponse::NotFoundFile;
        }
        let contents = contents.unwrap();
        FileDownloadResponse::FoundFile(FileData {
            num_chunks: file_c.0,
            contents,
            file_type: file_c.1,
            owner_key: file_c.2,
        })
    }

    /// Get the list of users that have access to the file by its [`FileId`]
    pub fn get_allowed_users(caller: Principal, file_id: &FileId) -> Vec<Principal> {
        if caller != Config::get_owner() {
            trap("Only the owner can get allowed users");
        }

        FileSharesStorage::get_file_shares_storage()
            .iter()
            .filter(|element| element.1.contains(file_id))
            .map(|(user_principal, _file_vector)| *user_principal)
            .collect()
    }

    /// Get [`FileStatus`] of the file by its [`FileId`]
    pub fn get_file_status(file_id: &FileId) -> FileStatus {
        // unwrap is safe, we know the file exists
        let file = &FileDataStorage::get_file(file_id).unwrap();
        match &file.content {
            FileContent::Pending { alias } => FileStatus::Pending {
                alias: alias.clone(),
                requested_at: file.metadata.requested_at,
            },
            FileContent::PartiallyUploaded { .. } => FileStatus::PartiallyUploaded,
            FileContent::Uploaded {
                owner_key: own_key, ..
            } => FileStatus::Uploaded {
                uploaded_at: file.metadata.uploaded_at.unwrap(),
                document_key: *own_key,
            },
        }
    }

    /// Get the list of files shared with the user by its [`Principal`]
    pub fn get_shared_files(caller: Principal, user_id: Principal) -> Vec<PublicFileMetadata> {
        if caller != Config::get_owner() {
            trap("Only the owner can get allowed users");
        }
        match FileSharesStorage::get_file_shares(&user_id) {
            None => vec![],
            Some(file_ids) => file_ids
                .iter()
                .map(|file_id| PublicFileMetadata {
                    file_id: *file_id,
                    file_name: FileDataStorage::get_file(file_id)
                        .expect("file must exist")
                        .metadata
                        .file_name
                        .clone(),
                    shared_with: Self::get_allowed_users(caller, file_id),
                    file_status: Self::get_file_status(file_id),
                })
                .collect(),
        }
    }

    /// Get the alias info by its alias as [`String`]
    pub fn get_alias_info(alias: String) -> Result<AliasInfo, GetAliasInfoError> {
        let Some(file_id) = FileAliasIndexStorage::get_file_id(&alias) else {
            return Err(GetAliasInfoError::NotFound);
        };

        let file = FileDataStorage::get_file(&file_id).unwrap();

        Ok(AliasInfo {
            file_id,
            file_name: file.metadata.file_name.clone(),
        })
    }

    /// Delete a file by its [`FileId`].
    ///
    /// The process of deleting a file is as follows:
    ///
    /// 1. Check whether the file exists in the storage.
    /// 2. Check if the file is shared with any users.
    /// 3. If the file is shared, remove the sharing information from the storage and revoke the sharing on the orchestrator.
    /// 4. If the file is being uploaded, remove the file request
    /// 5. Remove the file from the storage.
    pub async fn delete_file(caller: Principal, file_id: FileId) -> DeleteFileResponse {
        if caller != Config::get_owner() {
            trap("Only the owner can delete files");
        }

        // 1. Check whether the file exists in the storage.
        let Some(file) = FileDataStorage::get_file(&file_id) else {
            return DeleteFileResponse::FileNotFound;
        };

        // 2. Check if the file is shared with any users.
        let users_with_access = FileSharesStorage::get_users_with_file_shares(&file_id);
        // revoke share on orchestrator
        if cfg!(target_family = "wasm") {
            if let Err(err) = OrchestratorClient::from(Config::get_orchestrator())
                .revoke_share_file_for_users(&users_with_access, file_id)
                .await
            {
                return DeleteFileResponse::FailedToRevokeShare(err.to_string());
            }
        }

        // 3. If the file is shared, remove the sharing information from the storage
        for user_id in users_with_access {
            // remove file from user shares
            FileSharesStorage::revoke(&user_id, &file_id);
        }
        // remove file
        FileDataStorage::remove_file(&file_id);
        OwnedFilesStorage::remove_owned_file(&file_id);
        // remove file content / alias
        match file.content {
            FileContent::PartiallyUploaded { num_chunks, .. }
            | FileContent::Uploaded { num_chunks, .. } => {
                for chunk_id in 0..num_chunks {
                    FileContentsStorage::remove_file_contents(&file_id, &chunk_id);
                }
            }
            FileContent::Pending { alias } => {
                FileAliasIndexStorage::remove_file_id(&alias);
            }
        }

        DeleteFileResponse::Ok
    }
}

#[cfg(test)]
mod test {
    use candid::Principal;
    use did::user_canister::UserCanisterInitArgs;

    use super::*;

    #[test]
    fn test_should_init_canister() {
        let orchestrator = Principal::from_slice(&[0, 1, 2, 3]);
        let owner = Principal::from_slice(&[4, 5, 6, 7]);
        Canister::init(UserCanisterInstallArgs::Init(UserCanisterInitArgs {
            orchestrator,
            owner,
        }));

        assert_eq!(Config::get_orchestrator(), orchestrator);
        assert_eq!(Config::get_owner(), owner);
    }

    #[tokio::test]
    async fn test_should_request_file() {
        let file_name = "test_file.txt".to_string();
        let caller = init();
        let alias = Canister::request_file(caller, file_name.clone()).await;
        // NOTE: we expect it to end with 0 because on unit tests the randomness is just zero.
        assert!(alias.ends_with("7000-8000-000000000000"));
    }

    #[tokio::test]
    #[should_panic(expected = "Only the owner can request a file")]
    async fn test_should_not_request_file_if_not_owner() {
        let file_name = "test_file.txt".to_string();
        init();
        Canister::request_file(Principal::anonymous(), file_name).await;
    }

    #[tokio::test]
    async fn test_should_get_requests() {
        let file_name = "test_file.txt";
        let caller = init();
        Canister::request_file(caller, file_name).await;
        let requests = Canister::get_requests(caller);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].file_name, file_name);
    }

    #[tokio::test]
    #[should_panic(expected = "Only the owner can get requests for a file")]
    async fn test_should_not_get_requests_if_not_owner() {
        let file_name = "test_file.txt";
        let caller = init();
        Canister::request_file(caller, file_name).await;
        Canister::get_requests(Principal::anonymous());
    }

    #[tokio::test]
    async fn test_should_upload_file() {
        let file_name = "test_file.txt";
        let caller = init();
        let alias = Canister::request_file(caller, file_name).await;
        let file_id = FileAliasIndexStorage::get_file_id(&alias).unwrap();
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;
        let result = Canister::upload_file(
            file_id,
            file_content.clone(),
            file_type.clone(),
            owner_key,
            num_chunks,
        );
        assert!(result.is_ok());
        let file = FileDataStorage::get_file(&file_id).unwrap();
        assert_eq!(
            file.content,
            FileContent::Uploaded {
                file_type,
                owner_key,
                shared_keys: BTreeMap::new(),
                num_chunks,
            }
        );
    }

    #[test]
    fn test_should_upload_file_atomic() {
        let caller = init();
        let file_name = "test_file.txt";
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;
        let file_id = Canister::upload_file_atomic(
            caller,
            UploadFileAtomicRequest {
                name: file_name.to_string(),
                content: file_content.clone(),
                file_type,
                owner_key,
                num_chunks,
            },
        );
        assert_eq!(file_id, 0);

        // Check if the file was uploaded correctly
        let file = FileDataStorage::get_file(&file_id).unwrap();
        assert_eq!(
            file.content,
            FileContent::Uploaded {
                file_type: "text/plain".to_string(),
                owner_key,
                shared_keys: BTreeMap::new(),
                num_chunks,
            }
        );
        // Check if the file content was stored correctly
        let file_content_stored = FileContentsStorage::get_file_contents(&file_id, &0).unwrap();
        assert_eq!(file_content_stored, file_content);
    }

    #[test]
    #[should_panic(expected = "Only the owner can upload a file")]
    fn test_should_not_upload_file_atomic_if_not_owner() {
        init();
        let file_name = "test_file.txt";
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;
        Canister::upload_file_atomic(
            Principal::anonymous(),
            UploadFileAtomicRequest {
                name: file_name.to_string(),
                content: file_content,
                file_type,
                owner_key,
                num_chunks,
            },
        );
    }

    #[test]
    fn test_should_upload_file_continue() {
        let caller = init();
        let file_name = "test_file.txt";
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 2;
        let file_id = Canister::upload_file_atomic(
            caller,
            UploadFileAtomicRequest {
                name: file_name.to_string(),
                content: file_content.clone(),
                file_type,
                owner_key,
                num_chunks,
            },
        );
        assert_eq!(file_id, 0);

        // Check if the file was uploaded correctly
        let file = FileDataStorage::get_file(&file_id).unwrap();
        let mut uploaded_chunks = UploadedChunks::default();
        uploaded_chunks.insert(0);
        assert_eq!(
            file.content,
            FileContent::PartiallyUploaded {
                num_chunks,
                uploaded_chunks,
                file_type: "text/plain".to_string(),
                owner_key,
                shared_keys: BTreeMap::new(),
            }
        );

        // Upload the second chunk
        let result = Canister::upload_file_continue(UploadFileContinueRequest {
            file_id,
            chunk_id: 1,
            contents: vec![4, 5, 6],
        });
        assert_eq!(result, UploadFileContinueResponse::Ok);

        // Check if the file content was stored correctly
        let file_content_stored_0 = FileContentsStorage::get_file_contents(&file_id, &0).unwrap();
        assert_eq!(file_content_stored_0, vec![1, 2, 3]);

        let file_content_stored_1 = FileContentsStorage::get_file_contents(&file_id, &1).unwrap();
        assert_eq!(file_content_stored_1, vec![4, 5, 6]);
        // Check if the file content was updated correctly
        let file = FileDataStorage::get_file(&file_id).unwrap();
        assert_eq!(
            file.content,
            FileContent::Uploaded {
                file_type: "text/plain".to_string(),
                owner_key,
                shared_keys: BTreeMap::new(),
                num_chunks,
            }
        );
    }

    #[test]
    fn test_should_upload_file_continue_arbitrary_order_and_eval_responses() {
        let caller = init();
        let file_name = "test_file.txt";
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 6;

        // Check response for unknown file
        let result = Canister::upload_file_continue(UploadFileContinueRequest {
            file_id: 0,
            chunk_id: 1,
            contents: vec![4, 5, 6],
        });
        assert_eq!(result, UploadFileContinueResponse::FileNotFound);

        let file_id = Canister::upload_file_atomic(
            caller,
            UploadFileAtomicRequest {
                name: file_name.to_string(),
                content: file_content.clone(),
                file_type,
                owner_key,
                num_chunks,
            },
        );
        assert_eq!(file_id, 0);

        // Upload chunks in arbitrary order
        Canister::upload_file_continue(UploadFileContinueRequest {
            file_id,
            chunk_id: 3,
            contents: vec![10, 11, 12],
        });
        Canister::upload_file_continue(UploadFileContinueRequest {
            file_id,
            chunk_id: 1,
            contents: vec![4, 5, 6],
        });

        // Upload a duplicate chunk
        let result = Canister::upload_file_continue(UploadFileContinueRequest {
            file_id,
            chunk_id: 1,
            contents: vec![4, 5, 6],
        });
        assert_eq!(result, UploadFileContinueResponse::ChunkAlreadyUploaded);

        //Check out of bounds chunk
        let result = Canister::upload_file_continue(UploadFileContinueRequest {
            file_id,
            chunk_id: 6,
            contents: vec![19, 20, 21],
        });
        assert_eq!(result, UploadFileContinueResponse::ChunkOutOfBounds);

        // Upload the remaining chunks
        Canister::upload_file_continue(UploadFileContinueRequest {
            file_id,
            chunk_id: 5,
            contents: vec![16, 17, 18],
        });
        Canister::upload_file_continue(UploadFileContinueRequest {
            file_id,
            chunk_id: 4,
            contents: vec![13, 14, 15],
        });
        Canister::upload_file_continue(UploadFileContinueRequest {
            file_id,
            chunk_id: 2,
            contents: vec![7, 8, 9],
        });

        // Check if the file content was stored correctly
        for i in 0..num_chunks {
            let file_content_stored = FileContentsStorage::get_file_contents(&file_id, &i).unwrap();
            assert_eq!(
                file_content_stored,
                vec![i as u8 * 3 + 1, i as u8 * 3 + 2, i as u8 * 3 + 3]
            );
        }
        // Check if the file content was updated correctly
        let file = FileDataStorage::get_file(&file_id).unwrap();
        assert_eq!(
            file.content,
            FileContent::Uploaded {
                file_type: "text/plain".to_string(),
                owner_key,
                shared_keys: BTreeMap::new(),
                num_chunks,
            }
        );

        // Check already uploaded file
        let result = Canister::upload_file_continue(UploadFileContinueRequest {
            file_id,
            chunk_id: 1,
            contents: vec![4, 5, 6],
        });
        assert_eq!(result, UploadFileContinueResponse::FileAlreadyUploaded);
    }

    #[tokio::test]
    async fn test_should_download_file() {
        let caller = init();
        let owner = init();
        let file_name = "test_file.txt";
        let alias = Canister::request_file(owner, file_name).await;
        let file_id = FileAliasIndexStorage::get_file_id(&alias).unwrap();
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;
        let _ = Canister::upload_file(
            file_id,
            file_content.clone(),
            file_type.clone(),
            owner_key,
            num_chunks,
        );
        // Download the file as the owner
        let result = Canister::download_file(owner, file_id, 0);
        assert_eq!(
            result,
            FileDownloadResponse::FoundFile(FileData {
                contents: file_content.clone(),
                file_type: file_type.clone(),
                owner_key,
                num_chunks
            })
        );
        // Download the file as a shared user
        let user_id = Principal::from_slice(&[4, 5, 6, 7]);
        let file_key_encrypted_for_user = [6; OwnerKey::KEY_SIZE].into();
        Canister::share_file(caller, user_id, file_id, file_key_encrypted_for_user).await;
        let result = Canister::download_file(user_id, file_id, 0);
        assert_eq!(
            result,
            FileDownloadResponse::FoundFile(FileData {
                contents: file_content,
                file_type,
                owner_key: [6; OwnerKey::KEY_SIZE].into(),
                num_chunks
            })
        );
    }

    #[tokio::test]
    async fn test_should_not_download_file_if_not_uploaded() {
        let caller = init();
        let owner = init();
        let file_name = "test_file.txt";
        let alias = Canister::request_file(owner, file_name).await;
        let file_id = FileAliasIndexStorage::get_file_id(&alias).unwrap();
        // Attempt to download the file on pending state
        let result = Canister::download_file(caller, file_id, 0);
        assert_eq!(result, FileDownloadResponse::NotUploadedFile);

        // Attempt to download the file on partially uploaded state
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 2;
        let _ = Canister::upload_file(
            file_id,
            file_content.clone(),
            file_type.clone(),
            owner_key,
            num_chunks,
        );
        let result = Canister::download_file(caller, file_id, 0);
        assert_eq!(result, FileDownloadResponse::NotUploadedFile);
    }

    #[tokio::test]
    async fn test_should_not_download_file_if_not_owner() {
        let caller = init();
        let owner = init();
        let file_name = "test_file.txt";
        let alias = Canister::request_file(owner, file_name).await;
        let file_id = FileAliasIndexStorage::get_file_id(&alias).unwrap();
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;
        let _ = Canister::upload_file(
            file_id,
            file_content.clone(),
            file_type.clone(),
            owner_key,
            num_chunks,
        );

        let user_id = Principal::from_slice(&[4, 5, 6, 7]);
        let file_key_encrypted_for_user = [6; OwnerKey::KEY_SIZE].into();
        Canister::share_file(caller, user_id, file_id, file_key_encrypted_for_user).await;

        let res = Canister::download_file(Principal::anonymous(), file_id, 0);
        assert_eq!(res, FileDownloadResponse::PermissionError);
    }

    #[tokio::test]
    async fn test_should_share_a_file() {
        let file_name = "test_file.txt";
        let caller = init();
        let alias = Canister::request_file(caller, file_name).await;
        let file_id = FileAliasIndexStorage::get_file_id(&alias).unwrap();
        let user_id = Principal::from_slice(&[4, 5, 6, 7]);
        let file_key_encrypted_for_user = [0; OwnerKey::KEY_SIZE].into();
        let result =
            Canister::share_file(caller, user_id, file_id, file_key_encrypted_for_user).await;
        assert_eq!(result, FileSharingResponse::PendingError);
        // Upload the file first
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;
        let res = Canister::upload_file(
            file_id,
            file_content,
            file_type.clone(),
            owner_key,
            num_chunks,
        );
        assert!(res.is_ok());
        // Now share the file
        let result =
            Canister::share_file(caller, user_id, file_id, file_key_encrypted_for_user).await;
        assert_eq!(result, FileSharingResponse::Ok);
        let file = FileDataStorage::get_file(&file_id).unwrap();

        assert_eq!(
            file.content,
            FileContent::Uploaded {
                file_type: file_type.clone(),
                owner_key,
                shared_keys: BTreeMap::from([(user_id, file_key_encrypted_for_user)]),
                num_chunks,
            }
        );
        // Check if the file is shared with the user
        let shared_files = Canister::get_shared_files(caller, user_id);
        assert_eq!(shared_files.len(), 1);
        assert_eq!(shared_files[0].file_id, file_id);
    }

    #[tokio::test]
    async fn test_should_share_more_than_a_file() {
        let file_name = "test_file.txt";
        let caller = init();

        let user_id = Principal::from_slice(&[4, 5, 6, 7]);
        let mut file_ids = vec![];
        for _ in 0..3 {
            let alias = Canister::request_file(caller, file_name).await;
            let file_id = FileAliasIndexStorage::get_file_id(&alias).unwrap();
            let file_key_encrypted_for_user = [0; OwnerKey::KEY_SIZE].into();
            let result =
                Canister::share_file(caller, user_id, file_id, file_key_encrypted_for_user).await;
            assert_eq!(result, FileSharingResponse::PendingError);
            // Upload the file first
            let file_content = vec![1, 2, 3];
            let file_type = "text/plain".to_string();
            let owner_key = [0; OwnerKey::KEY_SIZE].into();
            let num_chunks = 1;
            let res = Canister::upload_file(
                file_id,
                file_content,
                file_type.clone(),
                owner_key,
                num_chunks,
            );
            assert!(res.is_ok());
            // Now share the file
            let result =
                Canister::share_file(caller, user_id, file_id, file_key_encrypted_for_user).await;
            assert_eq!(result, FileSharingResponse::Ok);
            let file = FileDataStorage::get_file(&file_id).unwrap();

            assert_eq!(
                file.content,
                FileContent::Uploaded {
                    file_type: file_type.clone(),
                    owner_key,
                    shared_keys: BTreeMap::from([(user_id, file_key_encrypted_for_user)]),
                    num_chunks,
                }
            );

            file_ids.push(file_id);
        }

        // Check if the file is shared with the user
        let shared_files = Canister::get_shared_files(caller, user_id);
        assert_eq!(shared_files.len(), 3);
        assert!(file_ids.contains(&shared_files[0].file_id));
        assert!(file_ids.contains(&shared_files[1].file_id));
        assert!(file_ids.contains(&shared_files[2].file_id));
    }

    #[tokio::test]
    async fn should_share_file_with_users() {
        let file_name = "test_file.txt";
        let caller = init();
        let alias = Canister::request_file(caller, file_name).await;
        let file_id = FileAliasIndexStorage::get_file_id(&alias).unwrap();
        //upload the file first
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;
        let res = Canister::upload_file(
            file_id,
            file_content,
            file_type.clone(),
            owner_key,
            num_chunks,
        );
        assert!(res.is_ok());
        // Now share the file with multiple users

        let user_ids = vec![
            Principal::from_slice(&[4, 5, 6, 7]),
            Principal::from_slice(&[8, 9, 10, 11]),
        ];
        let file_key_encrypted_for_user = vec![
            [2; OwnerKey::KEY_SIZE].into(),
            [1; OwnerKey::KEY_SIZE].into(),
        ];
        Canister::share_file_with_users(
            caller,
            user_ids.clone(),
            file_id,
            file_key_encrypted_for_user,
        )
        .await;
        for user_id in user_ids {
            let shared_files = Canister::get_shared_files(caller, user_id);
            assert_eq!(shared_files.len(), 1);
            assert_eq!(shared_files[0].file_id, file_id);
        }
    }

    #[tokio::test]
    #[should_panic(expected = "Only the owner can share a file")]
    async fn test_only_owner_should_share_file() {
        init();
        let user_id = Principal::from_slice(&[4, 5, 6, 7]);
        let file_id = 1;
        let file_key_encrypted_for_user = [0; OwnerKey::KEY_SIZE].into();
        Canister::share_file(
            Principal::anonymous(),
            user_id,
            file_id,
            file_key_encrypted_for_user,
        )
        .await;
    }

    #[tokio::test]
    async fn test_should_revoke_file_sharing() {
        let file_name = "test_file.txt";
        let caller = init();
        let alias = Canister::request_file(caller, file_name).await;
        let file_id = FileAliasIndexStorage::get_file_id(&alias).unwrap();
        //upload the file first
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;
        let res = Canister::upload_file(
            file_id,
            file_content,
            file_type.clone(),
            owner_key,
            num_chunks,
        );
        assert!(res.is_ok());
        // Now share the file with  user
        let user_id = Principal::from_slice(&[4, 5, 6, 7]);
        let file_key_encrypted_for_user = [0; OwnerKey::KEY_SIZE].into();
        Canister::share_file(caller, user_id, file_id, file_key_encrypted_for_user).await;
        // Revoke sharing
        Canister::revoke_file_sharing(caller, user_id, file_id).await;
        // Check if the user can still access the shared files
        let shared_files = Canister::get_shared_files(caller, user_id);
        assert_eq!(shared_files.len(), 0);
        // check if file has its sharing revoked
        let file = FileDataStorage::get_file(&file_id).unwrap();
        assert_eq!(
            file.content,
            FileContent::Uploaded {
                file_type: file_type.clone(),
                owner_key,
                shared_keys: BTreeMap::new(),
                num_chunks,
            }
        );
    }

    #[tokio::test]
    async fn test_should_revoke_file_sharing_when_more_files_are_shared() {
        let file_name = "test_file.txt";
        let caller = init();
        let mut file_ids = vec![];
        let user_id = Principal::from_slice(&[4, 5, 6, 7]);
        let file_key_encrypted_for_user = [0; OwnerKey::KEY_SIZE].into();
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;

        for _ in 0..5 {
            let alias = Canister::request_file(caller, file_name).await;
            let file_id = FileAliasIndexStorage::get_file_id(&alias).unwrap();
            //upload the file first
            let file_content = vec![1, 2, 3];

            let res = Canister::upload_file(
                file_id,
                file_content,
                file_type.clone(),
                owner_key,
                num_chunks,
            );
            assert!(res.is_ok());
            // Now share the file with  user
            Canister::share_file(caller, user_id, file_id, file_key_encrypted_for_user).await;

            file_ids.push(file_id);
        }

        let file_id = file_ids[1];
        // Revoke sharing
        Canister::revoke_file_sharing(caller, user_id, file_id).await;
        // Check if the user can still access the shared files
        let shared_files = Canister::get_shared_files(caller, user_id);
        assert_eq!(shared_files.len(), file_ids.len() - 1);
        // check if file has its sharing revoked
        let file = FileDataStorage::get_file(&file_id).unwrap();
        assert_eq!(
            file.content,
            FileContent::Uploaded {
                file_type: file_type.clone(),
                owner_key,
                shared_keys: BTreeMap::new(),
                num_chunks,
            }
        );
    }

    #[tokio::test]
    #[should_panic(expected = "Only the owner can revoke file sharing")]
    async fn test_only_owner_should_revoke_file_sharing() {
        init();
        let user_id = Principal::from_slice(&[4, 5, 6, 7]);
        let file_id = 1;
        Canister::revoke_file_sharing(Principal::anonymous(), user_id, file_id).await;
    }

    #[tokio::test]
    async fn test_should_get_alias_info() {
        let file_name = "test_file.txt";
        let caller = init();
        let alias = Canister::request_file(caller, file_name).await;
        let alias_info = Canister::get_alias_info(alias.clone());
        assert!(alias_info.is_ok());
        let alias_info = alias_info.unwrap();
        assert_eq!(alias_info.file_name, file_name);
    }

    #[test]
    fn test_should_get_alias_info_not_found() {
        init();
        let alias = "non_existent_alias".to_string();
        let alias_info = Canister::get_alias_info(alias);
        assert!(alias_info.is_err());
        assert_eq!(alias_info.unwrap_err(), GetAliasInfoError::NotFound);
    }

    #[tokio::test]
    async fn test_should_delete_file() {
        let user = init();

        let file_name = "test_file.txt";
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 1;
        let file_id = Canister::upload_file_atomic(
            user,
            UploadFileAtomicRequest {
                name: file_name.to_string(),
                content: file_content,
                file_type,
                owner_key,
                num_chunks,
            },
        );

        // share file with alice
        let alice = Principal::from_slice(&[4, 5, 6, 7]);
        let file_key_encrypted_for_user = [0; OwnerKey::KEY_SIZE].into();
        let result = Canister::share_file(user, alice, file_id, file_key_encrypted_for_user).await;
        assert_eq!(result, FileSharingResponse::Ok);

        // delete file
        let resp = Canister::delete_file(user, file_id).await;
        assert_eq!(resp, DeleteFileResponse::Ok);

        // check if the file is deleted
        let file = FileDataStorage::get_file(&file_id);
        assert!(file.is_none());
        // check if the file is deleted from the alias index
        let resp_file_id = FileAliasIndexStorage::get_file_id(&file_name.to_string());
        assert!(resp_file_id.is_none());
        // check if the file is deleted from the content storage
        let file_content = FileContentsStorage::get_file_contents(&file_id, &0);
        assert!(file_content.is_none());
        // check if the file is deleted from the shares storage
        let shares = FileSharesStorage::get_file_shares(&alice);
        assert!(shares.is_none());
    }

    #[tokio::test]
    async fn test_should_delete_file_when_pending() {
        let user = init();

        let file_name = "test_file.txt";
        let file_content = vec![1, 2, 3];
        let file_type = "text/plain".to_string();
        let owner_key = [0; OwnerKey::KEY_SIZE].into();
        let num_chunks = 4;
        let request_id = Canister::request_file(user, file_name.to_string()).await;
        // upload the file first
        let file_id = FileAliasIndexStorage::get_file_id(&request_id).unwrap();
        let res = Canister::upload_file(
            file_id,
            file_content,
            file_type.clone(),
            owner_key,
            num_chunks,
        );
        assert!(res.is_ok());

        // share file with alice
        let alice = Principal::from_slice(&[4, 5, 6, 7]);
        let file_key_encrypted_for_user = [0; OwnerKey::KEY_SIZE].into();
        let result = Canister::share_file(user, alice, file_id, file_key_encrypted_for_user).await;
        assert_eq!(result, FileSharingResponse::Ok);

        // delete file
        let resp = Canister::delete_file(user, file_id).await;
        assert_eq!(resp, DeleteFileResponse::Ok);

        // check if the file is deleted
        let file = FileDataStorage::get_file(&file_id);
        assert!(file.is_none());
        // check if the file is deleted from the alias index
        let resp_file_id = FileAliasIndexStorage::get_file_id(&file_name.to_string());
        assert!(resp_file_id.is_none());
        // check if the file is deleted from the content storage
        let file_content = FileContentsStorage::get_file_contents(&file_id, &0);
        assert!(file_content.is_none());
        // check if the file is deleted from the shares storage
        let shares = FileSharesStorage::get_file_shares(&alice);
        assert!(shares.is_none());
    }

    #[tokio::test]
    #[should_panic(expected = "Only the owner can delete files")]
    async fn test_only_owner_should_delete_file() {
        init();
        let file_id = 1;
        Canister::delete_file(Principal::anonymous(), file_id).await;
    }

    fn init() -> Principal {
        let caller = Principal::from_slice(&[0, 1, 2, 3]);
        Canister::init(UserCanisterInstallArgs::Init(UserCanisterInitArgs {
            orchestrator: Principal::from_slice(&[0, 1, 2, 3]),
            owner: caller,
        }));

        caller
    }
}
