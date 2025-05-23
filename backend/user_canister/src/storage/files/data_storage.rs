use super::{FILE_DATA_STORAGE, File, FileId, with_file_data};

// Public API for the file data storage
pub struct FileDataStorage;

impl FileDataStorage {
    /// Get a file by its ID
    pub fn get_file(file_id: &FileId) -> Option<File> {
        with_file_data(file_id, |file| file)
    }

    /// Set a file by its ID
    pub fn set_file(file_id: &FileId, file: File) {
        FILE_DATA_STORAGE.with_borrow_mut(|file_data| {
            file_data.insert(*file_id, file);
        });
    }

    /// Remove a file by its ID
    pub fn remove_file(file_id: &FileId) {
        FILE_DATA_STORAGE.with_borrow_mut(|file_data| {
            file_data.remove(file_id);
        });
    }
}

#[cfg(test)]
mod test {

    use candid::Principal;

    use super::*;
    use crate::storage::files::{FileContent, FileMetadata};

    #[test]
    fn test_file_data_storage() {
        let file_id = 1;
        let file = File {
            metadata: FileMetadata {
                file_name: "test_file".to_string(),
                user_public_key: vec![0; 32].try_into().unwrap(),
                requester_principal: Principal::from_slice(&[1; 29]),
                requested_at: 0,
                uploaded_at: None,
            },
            content: FileContent::Pending {
                alias: "test_alias".to_string(),
            },
        };
        FileDataStorage::set_file(&file_id, file.clone());
        assert_eq!(FileDataStorage::get_file(&file_id), Some(file));

        assert!(FileDataStorage::get_file(&2).is_none());
    }

    #[test]
    fn test_remove_file() {
        let file_id = 1;
        let file = File {
            metadata: FileMetadata {
                file_name: "test_file".to_string(),
                user_public_key: vec![1; 32].try_into().unwrap(),
                requester_principal: Principal::from_slice(&[1; 29]),
                requested_at: 0,
                uploaded_at: None,
            },
            content: FileContent::Pending {
                alias: "test_alias".to_string(),
            },
        };
        FileDataStorage::set_file(&file_id, file.clone());
        assert_eq!(FileDataStorage::get_file(&file_id), Some(file));

        FileDataStorage::remove_file(&file_id);
        assert!(FileDataStorage::get_file(&file_id).is_none());
    }
}
