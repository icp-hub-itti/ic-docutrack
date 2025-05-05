use std::path::Path;

pub enum Canister {
    Orchestrator,
    UserCan,
    CyclesMinting,
    IcpIndex,
    IcpLedger,
    OrbitStation,
    OrbitUpgrader,
}

impl Canister {
    pub fn as_path(&self) -> &'static Path {
        match self {
            Canister::Orchestrator => Path::new("../.artifact/orchestrator.wasm.gz"),
            Canister::UserCan => Path::new("../.artifact/usercan.wasm.gz"),
            Canister::CyclesMinting => Path::new("../.artifact/cycles-minting-canister.wasm.gz"),
            Canister::IcpIndex => Path::new("../.artifact/icp-index.wasm.gz"),
            Canister::IcpLedger => Path::new("../.artifact/icp-ledger.wasm.gz"),
            Canister::OrbitStation => Path::new("../.artifact/orbit-station.wasm.gz"),
            Canister::OrbitUpgrader => Path::new("../.artifact/orbit-upgrader.wasm.gz"),
        }
    }
}
