use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};

use candid::Principal;
use did::user_canister::{OwnerKey, PublicKey};
use ic_stable_structures::Storable;
use ic_stable_structures::storable::Bound;

use crate::utils::trap;

pub const MAX_FILE_NAME_SIZE: usize = 255;
pub const MAX_PRINCIPAL_SIZE: usize = 29;

pub type ChunkId = u64;
pub type FileId = did::FileId;

/// A file is composed of its metadata [`FileMetadata`] and its content [`FileContent`], which is a blob.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct File {
    pub metadata: FileMetadata,
    pub content: FileContent,
}

// strategy [metadata_len: u16 | metadata_bytes | content_bytes]
impl Storable for File {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut bytes = Vec::new();

        // Encode metadata
        let metadata_bytes = self.metadata.to_bytes();
        let metadata_len = metadata_bytes.len();
        if metadata_len > u16::MAX as usize {
            trap("Metadata length exceeds u16::MAX");
        }
        bytes.extend_from_slice(&(metadata_len as u16).to_le_bytes());
        bytes.extend_from_slice(&metadata_bytes);

        // Encode content
        let content_bytes = self.content.to_bytes().into_owned();
        bytes.extend_from_slice(&content_bytes);

        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let mut offset = 0;

        // Read metadata_len
        if offset + 2 > bytes.len() {
            trap("Not enough bytes for metadata_len");
        }
        let metadata_len = u16::from_le_bytes(
            bytes[offset..offset + 2]
                .try_into()
                .expect("Failed to decode metadata_len"),
        ) as usize;
        offset += 2;

        // Read metadata
        if offset + metadata_len > bytes.len() {
            trap("Not enough bytes for metadata");
        }
        let metadata =
            FileMetadata::from_bytes(Cow::Borrowed(&bytes[offset..offset + metadata_len]));
        offset += metadata_len;

        // Read content (remaining bytes)
        if offset > bytes.len() {
            trap("Not enough bytes for content");
        }
        let content = FileContent::from_bytes(Cow::Borrowed(&bytes[offset..]));

        File { metadata, content }
    }
}

const OP_PENDING: u8 = 0;
const OP_UPLOADED: u8 = 1;
const OP_PARTIALLY_UPLOADED: u8 = 2;

/// The content of a file can be pending, uploaded, or partially uploaded.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FileContent {
    Pending {
        alias: String,
    },
    Uploaded {
        num_chunks: u64,
        file_type: String,
        owner_key: OwnerKey,
        shared_keys: BTreeMap<Principal, OwnerKey>,
    },
    PartiallyUploaded {
        num_chunks: u64,
        uploaded_chunks: UploadedChunks,
        file_type: String,
        owner_key: OwnerKey,
        shared_keys: BTreeMap<Principal, OwnerKey>,
    },
}

impl Storable for FileContent {
    const BOUND: Bound = Bound::Unbounded;

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        if bytes.is_empty() {
            trap(" Failed to decode FileContent: empty bytes");
        }

        let op_code = bytes[0];

        match op_code {
            OP_PENDING => Self::decode_pending(&bytes[1..]),
            OP_UPLOADED => Self::decode_uploaded(&bytes[1..]),
            OP_PARTIALLY_UPLOADED => Self::decode_partially_uploaded(&bytes[1..]),
            _ => trap("Failed to decode FileContent: invalid op code"),
        }
    }

    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        match self {
            FileContent::Pending { alias } => Self::encode_pending(alias).into(),
            FileContent::Uploaded {
                num_chunks,
                file_type,
                owner_key,
                shared_keys,
            } => Self::encode_uploaded(num_chunks, file_type, owner_key, shared_keys).into(),
            FileContent::PartiallyUploaded {
                num_chunks,
                uploaded_chunks,
                file_type,
                owner_key,
                shared_keys,
            } => Self::encode_partially_uploaded(
                num_chunks,
                uploaded_chunks,
                file_type,
                owner_key,
                shared_keys,
            )
            .into(),
        }
    }
}

impl FileContent {
    // Decode Variant  for [`Pending::{alias}`]
    fn decode_pending(bytes: &[u8]) -> FileContent {
        let alias_len = bytes[0] as usize;
        let alias_bytes = &bytes[1..1 + alias_len];
        let alias = String::from_utf8(alias_bytes.to_vec()).expect("Failed to decode alias");
        FileContent::Pending { alias }
    }

    //Encode Variant for [`Pending::{alias}`]
    fn encode_pending(alias: &String) -> Vec<u8> {
        let mut bytes = vec![OP_PENDING];
        // write alias len
        bytes.push(alias.len() as u8);
        // write alias
        bytes.extend_from_slice(alias.as_bytes());
        bytes
    }

    // Decode Variant for [`Uploaded::{num_chunks, file_type, owner_key, shared_keys}`]
    fn decode_uploaded(bytes: &[u8]) -> FileContent {
        let mut offset = 0;
        if bytes.is_empty() {
            trap("Not enough bytes for FileContent");
        }
        // Read num_chunks
        let num_chunks_len = bytes[offset] as usize;
        offset += 1;
        if offset + num_chunks_len > bytes.len() {
            trap("Not enough bytes for num_chunks ");
        }
        let num_chunks = u64::from_le_bytes(
            bytes[offset..offset + num_chunks_len]
                .try_into()
                .expect("Failed to decode num_chunks"),
        );
        offset += num_chunks_len;

        // Read file_type
        // one byte
        let file_type_len = bytes[offset] as usize;
        offset += 1;
        if offset + file_type_len > bytes.len() {
            trap("Not enough bytes for file_type");
        }
        let file_type = String::from_utf8(bytes[offset..offset + file_type_len].to_vec())
            .expect("Failed to decode file_type");
        offset += file_type_len;

        // Read owner_key
        if offset + OwnerKey::KEY_SIZE > bytes.len() {
            trap("Not enough bytes for owner_key");
        }
        let owner_key = OwnerKey::new(
            bytes[offset..offset + OwnerKey::KEY_SIZE]
                .try_into()
                .expect("Failed to decode owner_key"),
        );
        offset += OwnerKey::KEY_SIZE;

        // Read shared_keys (no length prefix, first 8 bytes are num_entries)
        let mut shared_keys = BTreeMap::new();
        if offset + 8 > bytes.len() {
            trap("Not enough bytes for num_entries");
        }
        let num_entries = u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .expect("Failed to decode num_entries"),
        ) as usize;
        offset += 8;

        for _ in 0..num_entries {
            // Read the principal length (u8)
            if offset + 1 > bytes.len() {
                trap("Not enough bytes for principal_len");
            }
            let principal_len = bytes[offset] as usize;
            offset += 1;

            if offset + principal_len > bytes.len() {
                trap("Not enough bytes for principal");
            }
            let principal = Principal::try_from(&bytes[offset..offset + principal_len])
                .expect("Failed to decode principal");
            offset += principal_len;

            if offset + OwnerKey::KEY_SIZE > bytes.len() {
                trap("Not enough bytes for encryption key");
            }
            let encryption_key = OwnerKey::new(
                bytes[offset..offset + OwnerKey::KEY_SIZE]
                    .try_into()
                    .expect("Failed to decode encryption key"),
            );
            offset += OwnerKey::KEY_SIZE;

            shared_keys.insert(principal, encryption_key);
        }

        FileContent::Uploaded {
            num_chunks,
            file_type,
            owner_key,
            shared_keys,
        }
    }

    //Encode Variant for [`Uploaded::{num_chunks, file_type, owner_key, shared_keys}`]
    fn encode_uploaded(
        num_chunks: &u64,
        file_type: &String,
        owner_key: &OwnerKey,
        shared_keys: &BTreeMap<Principal, OwnerKey>,
    ) -> Vec<u8> {
        let mut bytes = vec![OP_UPLOADED];

        // Write num_chunks(one byte for length)
        let num_chunks_bytes = num_chunks.to_le_bytes();
        bytes.push(num_chunks_bytes.len() as u8);
        bytes.extend_from_slice(&num_chunks_bytes);

        // Write file_type (one byte for length)
        bytes.push(file_type.len() as u8);
        bytes.extend_from_slice(file_type.as_bytes());

        // Write owner_key
        bytes.extend_from_slice(owner_key.as_bytes());

        // Write shared_keys (no length prefix , first 8 bytes are num_entries)
        let num_entries = shared_keys.len() as u64;
        bytes.extend_from_slice(&num_entries.to_le_bytes());
        for (principal, encryption_key) in shared_keys {
            let principal_bytes = principal.as_slice();
            let principal_len = principal_bytes.len();

            bytes.push(principal_len as u8);
            bytes.extend_from_slice(principal_bytes);
            bytes.extend_from_slice(encryption_key.as_bytes());
        }

        bytes
    }

    // Decode Variant for [`PartiallyUploaded::{num_chunks,uploaded_chunks, file_type, owner_key, shared_keys}`]
    fn decode_partially_uploaded(bytes: &[u8]) -> FileContent {
        let mut offset = 0;
        if bytes.is_empty() {
            trap("Not enough bytes for FileContent");
        }
        // Read num_chunks (u8)
        let num_chunks_len = bytes[offset] as usize;
        offset += 1;
        if offset + num_chunks_len > bytes.len() {
            trap("Not enough bytes for num_chunks ");
        }
        let num_chunks = u64::from_le_bytes(
            bytes[offset..offset + num_chunks_len]
                .try_into()
                .expect("Failed to decode num_chunks"),
        );
        offset += num_chunks_len;

        // Read uploaded_chunks len
        if offset + 8 > bytes.len() {
            trap("Not enough bytes for uploaded_chunks");
        }
        let uploaded_chunks_len = u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .expect("Failed to decode uploaded_chunks"),
        ) as usize;
        offset += 8;
        if offset + uploaded_chunks_len * 8 > bytes.len() {
            trap("Not enough bytes for uploaded_chunks");
        }

        let mut uploaded_chunks = UploadedChunks::default();
        for _ in 0..uploaded_chunks_len {
            // Read each chunk ID (u64)
            let chunk_id = u64::from_le_bytes(
                bytes[offset..offset + 8]
                    .try_into()
                    .expect("Failed to decode chunk_id"),
            );
            uploaded_chunks.insert(chunk_id);
            offset += 8;
        }

        // Read file_type
        // one byte
        let file_type_len = bytes[offset] as usize;
        offset += 1;
        if offset + file_type_len > bytes.len() {
            trap("Not enough bytes for file_type");
        }
        let file_type = String::from_utf8(bytes[offset..offset + file_type_len].to_vec())
            .expect("Failed to decode file_type");
        offset += file_type_len;

        // Read owner_key
        if offset + OwnerKey::KEY_SIZE > bytes.len() {
            trap("Not enough bytes for owner_key");
        }
        let owner_key = OwnerKey::new(
            bytes[offset..offset + OwnerKey::KEY_SIZE]
                .try_into()
                .expect("Failed to decode owner_key"),
        );
        offset += OwnerKey::KEY_SIZE;

        // Read shared_keys (no length prefix, first 8 bytes are num_entries)
        let mut shared_keys = BTreeMap::new();
        if offset + 8 > bytes.len() {
            trap("Not enough bytes for num_entries");
        }
        let num_entries = u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .expect("Failed to decode num_entries"),
        ) as usize;
        offset += 8;

        for _ in 0..num_entries {
            // Read the principal length (u8)
            if offset + 1 > bytes.len() {
                trap("Not enough bytes for principal_len");
            }
            let principal_len = bytes[offset] as usize;
            offset += 1;

            if offset + principal_len > bytes.len() {
                trap(
                    "
             Not enough bytes for principal",
                );
            }
            let principal = Principal::try_from(&bytes[offset..offset + principal_len])
                .expect("Failed to decode principal");
            offset += principal_len;

            if offset + OwnerKey::KEY_SIZE > bytes.len() {
                trap("Not enough bytes for encryption key");
            }
            let encryption_key = OwnerKey::new(
                bytes[offset..offset + OwnerKey::KEY_SIZE]
                    .try_into()
                    .expect("Failed to decode encryption key"),
            );
            offset += OwnerKey::KEY_SIZE;

            shared_keys.insert(principal, encryption_key);
        }

        FileContent::PartiallyUploaded {
            num_chunks,
            uploaded_chunks,
            file_type,
            owner_key,
            shared_keys,
        }
    }

    //Encode Variant for [`PartiallyUploaded::{num_chunks, file_type, owner_key, shared_keys}`]
    fn encode_partially_uploaded(
        num_chunks: &u64,
        uploaded_chunks: &UploadedChunks,
        file_type: &String,
        owner_key: &OwnerKey,
        shared_keys: &BTreeMap<Principal, OwnerKey>,
    ) -> Vec<u8> {
        let mut bytes = vec![OP_PARTIALLY_UPLOADED];

        // Write num_chunks(one byte for length)
        let num_chunks_bytes = num_chunks.to_le_bytes();
        bytes.push(num_chunks_bytes.len() as u8);
        bytes.extend_from_slice(&num_chunks_bytes);

        // Write uploaded_chunks (first 8 bytes are num_chunk_entries)
        let num_chunk_entries = uploaded_chunks.len() as u64;
        bytes.extend_from_slice(&num_chunk_entries.to_le_bytes());
        for chunk_id in uploaded_chunks.to_hashset() {
            bytes.extend_from_slice(&chunk_id.to_le_bytes());
        }

        // Write file_type (one byte for length)
        bytes.push(file_type.len() as u8);
        bytes.extend_from_slice(file_type.as_bytes());

        // Write owner_key
        bytes.extend_from_slice(owner_key.as_bytes());

        // Write shared_keys (no length prefix , first 8 bytes are num_entries)
        let num_entries = shared_keys.len() as u64;
        bytes.extend_from_slice(&num_entries.to_le_bytes());
        for (principal, encryption_key) in shared_keys {
            let principal_bytes = principal.as_slice();
            let principal_len = principal_bytes.len();

            bytes.push(principal_len as u8);
            bytes.extend_from_slice(principal_bytes);
            bytes.extend_from_slice(encryption_key.as_bytes());
        }

        bytes
    }
}

/// The uploaded chunks of a file.
/// This is a HashSet of chunk IDs.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UploadedChunks(HashSet<ChunkId>);
impl UploadedChunks {
    /// Returns the inner HashSet.
    pub fn to_hashset(&self) -> HashSet<ChunkId> {
        self.0.clone()
    }

    /// Inserts a chunk ID into the set.
    pub fn insert(&mut self, chunk_id: ChunkId) {
        self.0.insert(chunk_id);
    }

    /// Verify if a chunk ID is in the set.
    pub fn contains(&self, chunk_id: &ChunkId) -> bool {
        self.0.contains(chunk_id)
    }
    /// Returns the number of chunks in the set.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl From<Vec<ChunkId>> for UploadedChunks {
    fn from(vec: Vec<ChunkId>) -> Self {
        UploadedChunks(vec.into_iter().collect())
    }
}

impl Storable for UploadedChunks {
    const BOUND: Bound = Bound::Unbounded; // No fixed size limit

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        // Create a buffer to hold the serialized data
        let mut buf = Vec::new();

        // Serialize the length of the vector as a 64
        buf.extend_from_slice(&(self.len() as u64).to_le_bytes());

        // Serialize each Chunk (u64) in little-endian format
        for chunk in self.0.iter() {
            buf.extend_from_slice(&chunk.to_le_bytes());
        }

        Cow::Owned(buf)
    }
    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let mut offset = 0;

        // Read the length of the vector
        if offset + 8 > bytes.len() {
            trap("Not enough bytes for length");
        }
        let len = u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .expect("Failed to decode length"),
        ) as usize;
        offset += 8;

        // Check if there are enough bytes for the chunks
        if offset + len * 8 > bytes.len() {
            trap("Not enough bytes for chunks");
        }

        // Create a HashSet to store the chunks
        // Allocate the HashSet with the expected capacity
        let mut chunks = HashSet::with_capacity(len);

        for _ in 0..len {
            // Read each chunk ID (u64)
            let chunk_id = u64::from_le_bytes(
                bytes[offset..offset + 8].try_into().unwrap(), // 8 bytes for u64
            );
            chunks.insert(chunk_id);
            offset += 8;
        }

        UploadedChunks(chunks)
    }
}
/// File metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMetadata {
    pub file_name: String,
    pub user_public_key: PublicKey,
    pub requester_principal: Principal,
    pub requested_at: u64,
    pub uploaded_at: Option<u64>,
}

impl Storable for FileMetadata {
    /// 1 for file name length, up to 255 for file name, 32 for public key, 29 for principal, 8 for requested_at, 9 for uploaded_at
    const BOUND: Bound = Bound::Bounded {
        max_size: 1
            + MAX_FILE_NAME_SIZE as u32
            + PublicKey::BOUND.max_size()
            + MAX_PRINCIPAL_SIZE as u32
            + 8
            + 9,
        is_fixed_size: false,
    };

    /// Strategy [file_name_len: u8 | file_name | public_key | principal_len: u8 | principal | requested_at: u64 | uploaded_at: u64]
    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let mut offset = 0;
        let file_name_len: u8 = bytes[offset];
        offset += 1;
        if file_name_len as usize > MAX_FILE_NAME_SIZE {
            trap("File name length exceeds maximum size");
        }
        if offset + file_name_len as usize > bytes.len() {
            trap("Not enough bytes for file name");
        }
        // Read file name
        let file_name = String::from_utf8(bytes[offset..offset + file_name_len as usize].to_vec())
            .expect("Failed to decode file name");
        offset += file_name_len as usize;
        if offset + PublicKey::KEY_LEN_SIZE > bytes.len() {
            trap("Not enough bytes for public key");
        }
        // Read public key
        let user_public_key = PublicKey::from_bytes(bytes[offset..].into());
        offset += user_public_key.encoding_size();

        if offset + 1 > bytes.len() {
            trap("Not enough bytes for principal_len");
        }
        // Read principal length (u8)
        let principal_len = bytes[offset] as usize;
        offset += 1;
        if principal_len > MAX_PRINCIPAL_SIZE {
            trap("Principal length exceeds maximum size");
        }
        if offset + principal_len > bytes.len() {
            trap("Not enough bytes for principal");
        }
        // Read principal
        let requester_principal = Principal::try_from(&bytes[offset..offset + principal_len])
            .expect("Failed to decode principal");
        offset += principal_len;
        if offset + 8 > bytes.len() {
            trap("Not enough bytes for requested_at");
        }
        // Read requested_at
        let requested_at = u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .expect("Invalid requested_at size"),
        );
        offset += 8;
        // Read uploaded_at
        // Check if there are enough bytes for uploaded_at
        if offset + 1 > bytes.len() {
            trap("Not enough bytes for uploaded_at");
        }
        let uploaded_at_option = bytes[offset];
        offset += 1;
        // If uploaded_at is present, read it
        if uploaded_at_option == 0 {
            if offset + 8 > bytes.len() {
                trap("Not enough bytes for uploaded_at");
            }
            let uploaded_at = u64::from_le_bytes(
                bytes[offset..offset + 8]
                    .try_into()
                    .expect("Invalid uploaded_at size"),
            );

            FileMetadata {
                file_name,
                user_public_key,
                requester_principal,
                requested_at,
                uploaded_at: Some(uploaded_at),
            }
        } else {
            FileMetadata {
                file_name,
                user_public_key,
                requester_principal,
                requested_at,
                uploaded_at: None,
            }
        }
    }

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let file_name_len = self.file_name.len() as u8;
        let mut bytes = Vec::with_capacity(
            1 + file_name_len as usize
                + self.user_public_key.encoding_size()
                + MAX_PRINCIPAL_SIZE
                + 8
                + 9,
        );

        // encode file name
        bytes.push(file_name_len);
        bytes.extend_from_slice(self.file_name.as_bytes());

        // encode public key
        bytes.extend_from_slice(self.user_public_key.to_bytes().as_ref());

        // encode principal
        let principal_bytes = self.requester_principal.as_slice();
        bytes.push(principal_bytes.len() as u8);
        bytes.extend_from_slice(principal_bytes);
        // encode requested_at
        bytes.extend_from_slice(&self.requested_at.to_le_bytes());
        // encode uploaded_at

        if let Some(uploaded_at) = self.uploaded_at {
            bytes.push(0);
            bytes.extend_from_slice(&uploaded_at.to_le_bytes());
        } else {
            bytes.push(1);
        }

        bytes.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storable_file_metadata_roundtrip() {
        let file_metadata = FileMetadata {
            file_name: "test.txt".to_string(),
            user_public_key: vec![0; 32].try_into().unwrap(),
            requester_principal: Principal::from_slice(&[0, 1, 2, 3]),
            requested_at: 123456789,
            uploaded_at: Some(987654321),
        };
        let bytes = file_metadata.to_bytes();
        let deserialized = FileMetadata::from_bytes(bytes);
        assert_eq!(file_metadata, deserialized);
    }

    #[test]
    fn test_storable_file_content_roundtrip() {
        let file_content = FileContent::Uploaded {
            num_chunks: 5,
            file_type: "text/plain".to_string(),
            owner_key: [1; OwnerKey::KEY_SIZE].into(),
            shared_keys: BTreeMap::new(),
        };
        let bytes = file_content.to_bytes();
        let deserialized = FileContent::from_bytes(bytes);
        assert_eq!(file_content, deserialized);
    }

    #[test]
    fn test_storable_uploaded_chunks_roundtrip() {
        let mut uploaded_chunks = UploadedChunks::default();
        uploaded_chunks.insert(1);
        uploaded_chunks.insert(2);
        uploaded_chunks.insert(7);
        uploaded_chunks.insert(5);

        assert_eq!(uploaded_chunks.len(), 4);
        assert!(uploaded_chunks.0.contains(&1));
        assert!(uploaded_chunks.0.contains(&2));
        assert!(uploaded_chunks.0.contains(&7));
        assert!(uploaded_chunks.0.contains(&5));
        assert!(!uploaded_chunks.0.contains(&3));

        // Serialize and deserialize
        let serialized = uploaded_chunks.to_bytes();
        let deserialized = UploadedChunks::from_bytes(serialized);
        assert_eq!(uploaded_chunks.len(), deserialized.len());
        assert_eq!(uploaded_chunks.0, deserialized.0);
    }

    #[test]
    fn test_storable_file_roundtrip() {
        let file = File {
            metadata: FileMetadata {
                file_name: "test.txt".to_string(),
                user_public_key: vec![0; 32].try_into().unwrap(),
                requester_principal: Principal::from_slice(&[0; MAX_PRINCIPAL_SIZE]),
                requested_at: 123456789,
                uploaded_at: Some(987654321),
            },
            content: FileContent::Uploaded {
                num_chunks: 5,
                file_type: "text/plain".to_string(),
                owner_key: [1; OwnerKey::KEY_SIZE].into(),
                shared_keys: BTreeMap::new(),
            },
        };
        let bytes = file.to_bytes();
        let deserialized = File::from_bytes(bytes);
        assert_eq!(file, deserialized);
    }
}
