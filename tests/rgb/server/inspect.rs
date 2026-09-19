#![cfg(not(target_arch = "wasm32"))]

use crate::rgb::server::seeder::{
    ContractSeed, WalletSeed, create_new_contract, create_new_wallet,
};
use amplify::none;
use bitmask_core::rgb::get_contract;
use bitmask_core::wallet::watcher::watcher_inspect;

#[tokio::test]
async fn watcher_inspect_after_create_new_wallet() -> anyhow::Result<()> {
    let keys = create_new_wallet(WalletSeed::default()).await?;
    let sk = keys.private.nostr_prv.as_str();
    watcher_inspect(sk, "bitcoin").await?;
    watcher_inspect(sk, "RGB20").await?;
    Ok(())
}

#[tokio::test]
async fn get_contract_after_issue() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let sk = issuer_keys.private.nostr_prv.clone();
    let issue_res = create_new_contract(ContractSeed::fungible(issuer_keys, none!())).await?;
    assert_eq!(issue_res.len(), 1);
    let id = issue_res[0].contract_id.clone();
    watcher_inspect(&sk, "RGB20").await?;
    let got = get_contract(&sk, id.clone()).await?;
    assert_eq!(got.contract_id, id);
    assert_eq!(got.iface, issue_res[0].iface);
    assert_eq!(got.issue_utxo, issue_res[0].issue_utxo);
    Ok(())
}
