use std::path::Path;

pub enum Canister {
    Backend,
    OrbitStation,
}

impl Canister {
    pub fn as_path(&self) -> &'static Path {
        match self {
            Canister::Backend => Path::new("../.artifact/backend.wasm.gz"),
            Canister::OrbitStation => Path::new("../.artifact/orbit-station.wasm.gz"),
        }
    }
}
