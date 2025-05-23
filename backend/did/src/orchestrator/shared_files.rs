use std::collections::HashMap;

use candid::{CandidType, Principal};
use ic_stable_structures::Storable;
use ic_stable_structures::storable::Bound;
use serde::{Deserialize, Serialize};

use super::public_file_metadata::PublicFileMetadata;

/// File ID type
pub type FileId = u64;

/// Result for `share_file` and `share_file_with_users` methods
#[derive(Debug, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum ShareFileResponse {
    /// The file was shared successfully
    Ok,
    /// There is no user with the given principal
    NoSuchUser(Principal),
    /// Endpoint was not called by a user canister
    Unauthorized,
}

/// Result for `revoke_share_file` methods
#[derive(Debug, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum RevokeShareFileResponse {
    /// The file was unshared successfully
    Ok,
    /// There is no user with the given principal
    NoSuchUser(Principal),
    /// Endpoint was not called by a user canister
    Unauthorized,
}

/// Result for `shared_files` method
#[derive(Debug, Clone, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum SharedFilesResponse {
    /// List of shared files
    SharedFiles(HashMap<Principal, Vec<PublicFileMetadata>>),
    /// No such user
    NoSuchUser,
    /// Anonymous user
    AnonymousUser,
}

/// File metadata of a shared file
#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize, Serialize)]
pub struct ShareFileMetadata {
    pub file_name: String,
}

impl Storable for ShareFileMetadata {
    const BOUND: Bound = Bound::Unbounded;

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let len: u64 = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let file_name = String::from_utf8(bytes[8..(8 + len as usize)].to_vec()).unwrap();

        ShareFileMetadata { file_name }
    }

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let file_name_bytes = self.file_name.as_bytes();
        let len = file_name_bytes.len() as u64;
        let mut bytes = Vec::with_capacity(8 + len as usize);
        bytes.extend_from_slice(&len.to_le_bytes());
        bytes.extend_from_slice(file_name_bytes);
        bytes.into()
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_shared_files_roundtrip() {
        let file_name = "test_file.txt".to_string();
        let metadata = ShareFileMetadata { file_name };

        let bytes = metadata.to_bytes();
        let deserialized_metadata = ShareFileMetadata::from_bytes(bytes);

        assert_eq!(metadata, deserialized_metadata);
    }
}
