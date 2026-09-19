#![cfg(not(target_arch = "wasm32"))]

use crate::rgb::server::seeder::{
    ContractSeed, WalletSeed, create_new_contract, create_new_wallet,
};
use amplify::none;
use bitmask_core::rgb::import_contract;
use bitmask_core::structs::IssueResponse;

#[tokio::test]
async fn import_armored_rgb20_into_second_wallet() -> anyhow::Result<()> {
    let issuer = create_new_wallet(WalletSeed::default()).await?;
    let owner = create_new_wallet(WalletSeed::default()).await?;
    let issue_res = create_new_contract(ContractSeed::fungible(issuer, none!())).await?;
    assert_eq!(issue_res.len(), 1);
    let IssueResponse {
        contract,
        contract_id,
        iface,
        ..
    } = issue_res[0].clone();
    let imported = import_contract(&owner.private.nostr_prv, contract).await?;
    assert_eq!(imported.contract_id, contract_id);
    assert_eq!(imported.iface, iface);
    Ok(())
}

#[tokio::test]
async fn import_armored_rgb21_into_second_wallet() -> anyhow::Result<()> {
    let issuer = create_new_wallet(WalletSeed::default()).await?;
    let owner = create_new_wallet(WalletSeed::default()).await?;
    let mut issue_req = ContractSeed::uda(issuer, none!());
    issue_req.options.proxy = false;
    let issue_res = create_new_contract(issue_req).await?;
    assert_eq!(issue_res.len(), 1);
    let IssueResponse {
        contract,
        contract_id,
        iface,
        ..
    } = issue_res[0].clone();
    let imported = import_contract(&owner.private.nostr_prv, contract).await?;
    assert_eq!(imported.contract_id, contract_id);
    assert_eq!(imported.iface, iface);
    Ok(())
}
