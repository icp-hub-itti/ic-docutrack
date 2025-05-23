use candid::CandidType;
use serde::{Deserialize, Serialize};

use super::FileId;

/// Public file metadata which is stored for the shared files info
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicFileMetadata {
    pub file_id: FileId,
    pub file_name: String,
}
