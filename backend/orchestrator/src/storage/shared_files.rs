mod user_shared_files;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use candid::Principal;
use did::StorablePrincipal;
use did::orchestrator::{FileId, ShareFileMetadata};
use ic_stable_structures::memory_manager::VirtualMemory;
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap};

use self::user_shared_files::UserSharedFiles;
use crate::storage::memory::{
    MEMORY_MANAGER, SHARED_FILES_MEMORY_ID, SHARED_FILES_METADATA_MEMORY_ID,
    SHARED_FILES_METADATA_RC_MEMORY_ID,
};

thread_local! {
    /// Shared files. Maps users to their shared files, grouped by the user canister.
    static SHARED_FILES: RefCell<StableBTreeMap<StorablePrincipal, UserSharedFiles, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(SHARED_FILES_MEMORY_ID)))
    );

    /// Metadata for shared files. Maps userCanister AND file IDs to their metadata.
    static SHARED_FILES_METADATA: RefCell<StableBTreeMap<(StorablePrincipal, FileId), ShareFileMetadata, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(SHARED_FILES_METADATA_MEMORY_ID)))
    );

    /// Tracks the number of references to shared files metadata.
    ///
    /// This is used to determine if the metadata can be removed from the stable memory.
    static SHARED_FILES_METADATA_RC: RefCell<StableBTreeMap<(StorablePrincipal, FileId), u64, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableBTreeMap::new(MEMORY_MANAGER.with(|mm| mm.get(SHARED_FILES_METADATA_RC_MEMORY_ID)))
    );
}

/// Accessor for Storage for shared files.
///
/// Maps users to their shared files, grouped by the user canister.
pub struct SharedFilesStorage;

impl SharedFilesStorage {
    /// Share a file with a user for the provided user canister.
    ///
    /// Marks the file as shared for the user canister
    pub fn share_file(
        user: Principal,
        user_canister: Principal,
        file_id: FileId,
        metadata: ShareFileMetadata,
    ) {
        SHARED_FILES.with_borrow_mut(|shared_files| {
            let storable_user = StorablePrincipal::from(user);
            if !shared_files.contains_key(&storable_user) {
                shared_files.insert(storable_user, UserSharedFiles::default());
            }

            let mut user_shared_files = shared_files
                .get(&storable_user)
                .expect("user shared files must exist at this point");

            user_shared_files.insert_file(user_canister, file_id);

            shared_files.insert(storable_user, user_shared_files);
        });

        // insert the file metadata
        SHARED_FILES_METADATA.with_borrow_mut(|shared_files_metadata| {
            shared_files_metadata.insert((user_canister.into(), file_id), metadata);
        });

        // increment the reference count
        SHARED_FILES_METADATA_RC.with_borrow_mut(|shared_files_metadata_rc| {
            let rc = shared_files_metadata_rc
                .get(&(user_canister.into(), file_id))
                .unwrap_or(0);
            shared_files_metadata_rc.insert((user_canister.into(), file_id), rc + 1);
        });
    }

    /// Revoke a file share for a user for the provided user canister.
    pub fn revoke_share(user: Principal, user_canister: Principal, file_id: FileId) {
        SHARED_FILES.with_borrow_mut(|shared_files| {
            let storable_user = StorablePrincipal::from(user);
            if !shared_files.contains_key(&storable_user) {
                return;
            }

            let mut user_shared_files = shared_files
                .get(&storable_user)
                .expect("user shared files must exist at this point");

            user_shared_files.remove_file(user_canister, file_id);

            // If the user has no more files, remove the user from the map.
            if user_shared_files.is_empty() {
                shared_files.remove(&storable_user);
            } else {
                shared_files.insert(storable_user, user_shared_files);
            }
        });

        // decrement the reference count
        let rc = SHARED_FILES_METADATA_RC.with_borrow_mut(|shared_files_metadata_rc| {
            let rc = shared_files_metadata_rc
                .get(&(user_canister.into(), file_id))
                .unwrap_or(0)
                .checked_sub(1)
                .unwrap_or_default();
            if rc > 0 {
                shared_files_metadata_rc.insert((user_canister.into(), file_id), rc);
            } else {
                shared_files_metadata_rc.remove(&(user_canister.into(), file_id));
            }

            rc
        });

        // remove the file metadata if the reference count is 0
        if rc == 0 {
            SHARED_FILES_METADATA.with_borrow_mut(|shared_files_metadata| {
                shared_files_metadata.remove(&(user_canister.into(), file_id));
            });
        }
    }

    /// For a user, get the list of file IDs shared for each user canister.
    pub fn get_shared_files(user: Principal) -> HashMap<Principal, HashSet<FileId>> {
        SHARED_FILES.with_borrow(|shared_files| {
            let storable_user = StorablePrincipal::from(user);

            shared_files
                .get(&storable_user)
                .map(|user_shared_files| user_shared_files.get_files())
                .unwrap_or_default()
        })
    }

    /// Get the metadata for a file shared with a user.
    pub fn get_file_metadata(
        user_canister: Principal,
        file_id: FileId,
    ) -> Option<ShareFileMetadata> {
        SHARED_FILES_METADATA.with_borrow(|shared_files_metadata| {
            shared_files_metadata.get(&(user_canister.into(), file_id))
        })
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_insert_and_get_files_for_users() {
        let alice = Principal::from_slice(&[1; 29]);
        let bob = Principal::from_slice(&[2; 29]);

        let user_canister_a = Principal::from_slice(&[3; 29]);
        let user_canister_b = Principal::from_slice(&[4; 29]);

        // insert
        SharedFilesStorage::share_file(
            alice,
            user_canister_a,
            1,
            ShareFileMetadata {
                file_name: "test.txt".to_string(),
            },
        );
        SharedFilesStorage::share_file(
            alice,
            user_canister_b,
            2,
            ShareFileMetadata {
                file_name: "test.txt".to_string(),
            },
        );
        SharedFilesStorage::share_file(
            bob,
            user_canister_a,
            1,
            ShareFileMetadata {
                file_name: "test.txt".to_string(),
            },
        );

        // check
        let alice_files = SharedFilesStorage::get_shared_files(alice);
        assert_eq!(alice_files.len(), 2);
        assert!(alice_files.contains_key(&user_canister_a));
        assert!(alice_files.contains_key(&user_canister_b));
        assert!(alice_files[&user_canister_a].contains(&1));
        assert!(alice_files[&user_canister_b].contains(&2));

        let bob_files = SharedFilesStorage::get_shared_files(bob);
        assert_eq!(bob_files.len(), 1);
        assert!(bob_files.contains_key(&user_canister_a));
        assert!(bob_files[&user_canister_a].contains(&1));
    }

    #[test]
    fn test_should_revoke_file() {
        let alice = Principal::from_slice(&[1; 29]);
        let user_canister_a = Principal::from_slice(&[3; 29]);

        // insert
        SharedFilesStorage::share_file(
            alice,
            user_canister_a,
            1,
            ShareFileMetadata {
                file_name: "test.txt".to_string(),
            },
        );
        SharedFilesStorage::share_file(
            alice,
            user_canister_a,
            2,
            ShareFileMetadata {
                file_name: "test.txt".to_string(),
            },
        );

        // revoke
        SharedFilesStorage::revoke_share(alice, user_canister_a, 1);

        // check
        let alice_files = SharedFilesStorage::get_shared_files(alice);
        assert_eq!(alice_files.len(), 1);
        assert!(alice_files.contains_key(&user_canister_a));
        assert!(!alice_files[&user_canister_a].contains(&1));
        assert!(alice_files[&user_canister_a].contains(&2));

        // revoke the last file
        SharedFilesStorage::revoke_share(alice, user_canister_a, 2);

        // check
        let alice_files = SharedFilesStorage::get_shared_files(alice);
        assert_eq!(alice_files.len(), 0);
    }

    #[test]
    fn test_share_file_should_set_metadata_and_remove_them() {
        let alice = Principal::from_slice(&[1; 29]);
        let bob = Principal::from_slice(&[2; 29]);

        let user_canister_a = Principal::from_slice(&[3; 29]);

        SharedFilesStorage::share_file(
            alice,
            user_canister_a,
            1,
            ShareFileMetadata {
                file_name: "test.txt".to_string(),
            },
        );

        SharedFilesStorage::share_file(
            bob,
            user_canister_a,
            1,
            ShareFileMetadata {
                file_name: "test.txt".to_string(),
            },
        );

        // check metadata
        let metadata = SHARED_FILES_METADATA.with_borrow(|shared_files_metadata| {
            shared_files_metadata
                .get(&(user_canister_a.into(), 1))
                .unwrap()
        });
        assert_eq!(metadata.file_name, "test.txt".to_string());

        // check reference count
        let rc = SHARED_FILES_METADATA_RC.with_borrow(|shared_files_metadata_rc| {
            shared_files_metadata_rc
                .get(&(user_canister_a.into(), 1))
                .unwrap()
        });
        assert_eq!(rc, 2);

        // revoke for bob
        SharedFilesStorage::revoke_share(bob, user_canister_a, 1);
        // check reference count
        let rc = SHARED_FILES_METADATA_RC.with_borrow(|shared_files_metadata_rc| {
            shared_files_metadata_rc
                .get(&(user_canister_a.into(), 1))
                .unwrap()
        });
        assert_eq!(rc, 1);

        // check metadata
        let metadata = SHARED_FILES_METADATA.with_borrow(|shared_files_metadata| {
            shared_files_metadata
                .get(&(user_canister_a.into(), 1))
                .unwrap()
        });
        assert_eq!(metadata.file_name, "test.txt".to_string());

        // revoke for alice
        SharedFilesStorage::revoke_share(alice, user_canister_a, 1);
        // check reference count
        let rc = SHARED_FILES_METADATA_RC.with_borrow(|shared_files_metadata_rc| {
            shared_files_metadata_rc.get(&(user_canister_a.into(), 1))
        });
        assert!(rc.is_none());
        // check metadata
        let metadata = SHARED_FILES_METADATA.with_borrow(|shared_files_metadata| {
            shared_files_metadata.get(&(user_canister_a.into(), 1))
        });
        assert!(metadata.is_none());
    }
}
