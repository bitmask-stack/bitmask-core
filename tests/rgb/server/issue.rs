#![cfg(not(target_arch = "wasm32"))]

use crate::rgb::server::seeder::{
    ContractSeed, WalletSeed, create_new_contract, create_new_wallet, get_uri_media,
};
use amplify::none;

#[tokio::test]
async fn create_new_rgb20_contract() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;

    let issue_req = ContractSeed::fungible(issuer_keys, none!());
    let issue_res = create_new_contract(issue_req).await?;

    assert_eq!(issue_res.len(), 1);
    Ok(())
}

#[tokio::test]
async fn create_new_rgb21_contract() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;

    let mut issue_req = ContractSeed::uda(issuer_keys, none!());
    issue_req.options.proxy = false;

    let issue_res = create_new_contract(issue_req).await?;

    assert_eq!(issue_res.len(), 1);
    Ok(())
}

#[tokio::test]
async fn create_new_rgb21_contract_with_media() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;

    let issue_req = ContractSeed::uda(issuer_keys, Some(get_uri_media()));
    let issue_res = create_new_contract(issue_req).await?;

    assert_eq!(issue_res.len(), 1);
    let issue_res = issue_res[0].clone();

    assert!(issue_res.meta.is_some());
    Ok(())
}
