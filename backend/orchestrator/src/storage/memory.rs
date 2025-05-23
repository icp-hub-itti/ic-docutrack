use ic_stable_structures::DefaultMemoryImpl;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager as IcMemoryManager};

pub const ORBIT_STATION_MEMORY_ID: MemoryId = MemoryId::new(1);
pub const ORBIT_STATION_ADMIN_MEMORY_ID: MemoryId = MemoryId::new(2);

pub const USER_STORAGE_MEMORY_ID: MemoryId = MemoryId::new(10);
pub const USERNAMES_MEMORY_ID: MemoryId = MemoryId::new(11);

pub const USER_CANISTERS_MEMORY_ID: MemoryId = MemoryId::new(20);
pub const USER_CANISTERS_INDEX_MEMORY_ID: MemoryId = MemoryId::new(21);
pub const USER_CANISTER_CREATE_STATES_MEMORY_ID: MemoryId = MemoryId::new(22);

pub const SHARED_FILES_MEMORY_ID: MemoryId = MemoryId::new(30);
pub const SHARED_FILES_METADATA_MEMORY_ID: MemoryId = MemoryId::new(31);
pub const SHARED_FILES_METADATA_RC_MEMORY_ID: MemoryId = MemoryId::new(32);

thread_local! {
    /// Memory manager
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
}
