#![cfg(target_arch = "wasm32")]

use crate::rgb::web::seeder::{ContractSeed, WalletSeed, create_new_contract, create_new_wallet};
use amplify::none;
use bitmask_core::structs::IssueResponse;
use bitmask_core::web::rgb::{get_contract, import_contract};
use bitmask_core::web::wallet::destroy_watcher;
use bitmask_core::web::{json_parse, resolve};
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn import_rgb20_contract_into_second_wallet() -> anyhow::Result<()> {
    let issuer = create_new_wallet(WalletSeed::default()).await?;
    let owner = create_new_wallet(WalletSeed::default()).await?;

    let issue_req = ContractSeed::fungible(issuer.clone(), none!());
    let issue_res = create_new_contract(issue_req).await?;
    assert_eq!(issue_res.len(), 1);
    let issued = issue_res[0].clone();

    let got: IssueResponse = json_parse(
        &resolve(get_contract(
            issuer.private.nostr_prv.clone(),
            issued.contract_id.clone(),
        ))
        .await,
    );
    assert_eq!(got.contract_id, issued.contract_id);
    assert_eq!(got.iface, issued.iface);

    let imported: IssueResponse = json_parse(
        &resolve(import_contract(
            owner.private.nostr_prv.clone(),
            issued.contract.clone(),
        ))
        .await,
    );
    assert_eq!(imported.contract_id, issued.contract_id);
    assert_eq!(imported.iface, issued.iface);

    resolve(destroy_watcher(issuer.private.nostr_prv.to_string())).await;
    resolve(destroy_watcher(owner.private.nostr_prv.to_string())).await;
    Ok(())
}

#[wasm_bindgen_test]
async fn import_rgb21_contract_into_second_wallet() -> anyhow::Result<()> {
    let issuer = create_new_wallet(WalletSeed::default()).await?;
    let owner = create_new_wallet(WalletSeed::default()).await?;

    let issue_req = ContractSeed::uda(issuer.clone(), none!());
    let issue_res = create_new_contract(issue_req).await?;
    assert_eq!(issue_res.len(), 1);
    let issued = issue_res[0].clone();

    let got: IssueResponse = json_parse(
        &resolve(get_contract(
            issuer.private.nostr_prv.clone(),
            issued.contract_id.clone(),
        ))
        .await,
    );
    assert_eq!(got.contract_id, issued.contract_id);
    assert_eq!(got.iface, issued.iface);

    let imported: IssueResponse = json_parse(
        &resolve(import_contract(
            owner.private.nostr_prv.clone(),
            issued.contract.clone(),
        ))
        .await,
    );
    assert_eq!(imported.contract_id, issued.contract_id);
    assert_eq!(imported.iface, issued.iface);

    resolve(destroy_watcher(issuer.private.nostr_prv.to_string())).await;
    resolve(destroy_watcher(owner.private.nostr_prv.to_string())).await;
    Ok(())
}
