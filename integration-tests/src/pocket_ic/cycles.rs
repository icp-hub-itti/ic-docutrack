mod cmc;
mod icp_index;
mod icp_ledger;

use std::collections::{HashMap, HashSet};

use candid::{Encode, Principal};
use ic_ledger_types::{AccountIdentifier, DEFAULT_SUBACCOUNT, Tokens};

use super::PocketIcTestEnv;
use super::env::{PocketIc, update_candid_as};
use crate::actor::admin;
use crate::wasm::Canister;

const CYCLES_MINTING_CANISTER_ID: &str = "rkp4c-7iaaa-aaaaa-aaaca-cai";
const LEDGER_CANISTER_ID: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";
const NNS_EXCHANGE_RATE_CANISTER: &str = "uf6dk-hyaaa-aaaaq-qaaaq-cai";
const NNS_INDEX_CANISTER_ID: &str = "r7inp-6aaaa-aaaaa-aaabq-cai";
const NNS_LEDGER_CANISTER_ID: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";

const DEFAULT_CYCLES: u128 = 2_000_000_000_000_000;

pub async fn setup_cycles_minting_canister(pic: &PocketIc) {
    let nns_ledger_canister = install_nns_ledger_canister(pic).await;
    let _nns_index_canister = install_nns_index_canister(pic).await;

    let nns_cycles_ledger_canister_id =
        Principal::from_text("um5iw-rqaaa-aaaaq-qaaba-cai").unwrap();
    let nns_governance_canister_id = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
    let nns_exchange_rate_canister_id = install_nns_exchange_rate_canister(pic).await;

    let cmc_canister_id = install_cycles_minting_canister(
        pic,
        nns_ledger_canister,
        nns_governance_canister_id,
        nns_exchange_rate_canister_id,
        nns_cycles_ledger_canister_id,
    )
    .await;

    setup_cmc(pic, cmc_canister_id, nns_governance_canister_id).await;
}

async fn install_nns_exchange_rate_canister(pic: &PocketIc) -> Principal {
    let principal = Principal::from_text(NNS_EXCHANGE_RATE_CANISTER).unwrap();

    pic.create_canister_with_id(Some(admin()), None, principal)
        .await
        .expect("Failed to nns_exchange_rate canister");
    pic.add_cycles(principal, DEFAULT_CYCLES).await;

    principal
}

async fn install_nns_index_canister(pic: &PocketIc) -> Principal {
    let wasm_bytes = PocketIcTestEnv::load_wasm(Canister::IcpIndex);
    let principal =
        Principal::from_text(NNS_INDEX_CANISTER_ID).expect("Failed to parse nns index canister id");
    pic.create_canister_with_id(Some(admin()), None, principal)
        .await
        .expect("Failed to create index canister");
    pic.add_cycles(principal, DEFAULT_CYCLES).await;

    let nns_ledger_canister_id =
        Principal::from_text(LEDGER_CANISTER_ID).expect("Failed to parse nns ledger canister id");

    let arg = icp_index::NnsIndexCanisterInitPayload {
        ledger_id: nns_ledger_canister_id,
    };

    pic.install_canister(
        principal,
        wasm_bytes,
        Encode!(&arg).expect("failed to encode data"),
        Some(admin()),
    )
    .await;

    principal
}

async fn install_nns_ledger_canister(pic: &PocketIc) -> Principal {
    let wasm_bytes = PocketIcTestEnv::load_wasm(Canister::IcpLedger);
    let principal = Principal::from_text(NNS_LEDGER_CANISTER_ID)
        .expect("Failed to parse nns ledger canister id");
    pic.create_canister_with_id(Some(admin()), None, principal)
        .await
        .expect("Failed to create ledger canister");
    pic.add_cycles(principal, DEFAULT_CYCLES).await;

    let controller_account = AccountIdentifier::new(&admin(), &DEFAULT_SUBACCOUNT);
    let minting_account = AccountIdentifier::new(&admin(), &DEFAULT_SUBACCOUNT);

    let args =
        icp_ledger::NnsLedgerCanisterPayload::Init(icp_ledger::NnsLedgerCanisterInitPayload {
            minting_account: minting_account.to_string(),
            initial_values: HashMap::from([(
                controller_account.to_string(),
                Tokens::from_e8s(1_000_000_000_000),
            )]),
            send_whitelist: HashSet::new(),
            transfer_fee: Some(Tokens::from_e8s(10_000)),
            token_symbol: Some("ICP".to_string()),
            token_name: Some("Internet Computer".to_string()),
        });

    pic.install_canister(
        principal,
        wasm_bytes,
        Encode!(&args).expect("failed to encode data"),
        Some(admin()),
    )
    .await;

    principal
}

async fn install_cycles_minting_canister(
    pic: &PocketIc,
    nns_ledger_canister_id: Principal,
    nns_governance_canister_id: Principal,
    nns_exchange_rate_canister_id: Principal,
    nns_cycles_ledger_canister_id: Principal,
) -> Principal {
    let wasm_bytes = PocketIcTestEnv::load_wasm(Canister::CyclesMinting);

    let principal = Principal::from_text(CYCLES_MINTING_CANISTER_ID)
        .expect("Failed to parse cycles minting canister id");
    pic.create_canister_with_id(Some(admin()), None, principal)
        .await
        .expect("Failed to create cycles minting canister");
    pic.add_cycles(principal, DEFAULT_CYCLES).await;

    let arg = Some(cmc::CyclesCanisterInitPayload {
        ledger_canister_id: Some(nns_ledger_canister_id),
        governance_canister_id: Some(nns_governance_canister_id),
        minting_account_id: None,
        exchange_rate_canister: Some(cmc::ExchangeRateCanister::Set(
            nns_exchange_rate_canister_id,
        )),
        cycles_ledger_canister_id: Some(nns_cycles_ledger_canister_id),
        last_purged_notification: Some(0),
    });

    pic.install_canister(
        principal,
        wasm_bytes,
        Encode!(&arg).expect("failed to encode data"),
        Some(admin()),
    )
    .await;

    principal
}

async fn setup_cmc(
    pic: &PocketIc,
    cmc_canister_id: Principal,
    nns_governance_canister_id: Principal,
) {
    // set default (application) subnets on CMC
    // by setting authorized subnets associated with no principal (CMC API)
    let application_subnet_id = pic.topology().await.get_app_subnets()[0];
    let set_authorized_subnetwork_list_args = cmc::SetAuthorizedSubnetworkListArgs {
        who: None,
        subnets: vec![application_subnet_id],
    };
    update_candid_as::<_, ((),)>(
        pic,
        cmc_canister_id,
        nns_governance_canister_id,
        "set_authorized_subnetwork_list",
        (set_authorized_subnetwork_list_args,),
    )
    .await
    .expect("failed to set authorized subnetwork list");
    // add fiduciary subnet to CMC
    let update_subnet_type_args = cmc::UpdateSubnetTypeArgs::Add("fiduciary".to_string());
    update_candid_as::<_, ((),)>(
        pic,
        cmc_canister_id,
        nns_governance_canister_id,
        "update_subnet_type",
        (update_subnet_type_args,),
    )
    .await
    .expect("failed to update subnet type");
    let fiduciary_subnet_id = pic
        .topology()
        .await
        .get_fiduciary()
        .expect("failed to get fiduciary subnet");
    let change_subnet_type_assignment_args =
        cmc::ChangeSubnetTypeAssignmentArgs::Add(cmc::SubnetListWithType {
            subnets: vec![fiduciary_subnet_id],
            subnet_type: "fiduciary".to_string(),
        });
    update_candid_as::<_, ((),)>(
        pic,
        cmc_canister_id,
        nns_governance_canister_id,
        "change_subnet_type_assignment",
        (change_subnet_type_assignment_args,),
    )
    .await
    .expect("failed to change subnet type assignment");
}
