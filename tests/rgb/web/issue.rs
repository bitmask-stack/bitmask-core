#![cfg(target_arch = "wasm32")]

use crate::rgb::web::seeder::{ContractSeed, WalletSeed, create_new_contract, create_new_wallet};
use amplify::none;
use bitmask_core::web::resolve;
use bitmask_core::web::wallet::destroy_watcher;
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn create_new_rgb20_contract() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;

    let issue_req = ContractSeed::fungible(issuer_keys.clone(), none!());
    let issue_res = create_new_contract(issue_req).await?;

    assert_eq!(issue_res.len(), 1);

    resolve(destroy_watcher(issuer_keys.private.nostr_prv.to_string())).await;
    Ok(())
}
//
// #[wasm_bindgen_test]
// async fn create_new_rgb21_contract() -> anyhow::Result<()> {
//     let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
//
//     let issue_req = ContractSeed::uda(issuer_keys, none!());
//     let issue_res = create_new_contract(issue_req).await?;
//
//     assert_eq!(issue_res.iter().count(), 1);
//     Ok(())
// }
//
// #[wasm_bindgen_test]
// async fn create_new_rgb21_contract_with_media() -> anyhow::Result<()> {
//     let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
//
//     let issue_req = ContractSeed::uda(issuer_keys, Some(get_uri_media()));
//     let issue_res = create_new_contract_proxy(issue_req).await?;
//
//     assert_eq!(issue_res.iter().count(), 1);
//     Ok(())
// }
