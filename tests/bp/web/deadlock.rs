#![cfg(target_arch = "wasm32")]
use crate::bp::web::seeder::{create_new_wallet, WalletSeed};
use bitmask_core::web::rgb::{destroy_watcher, list_transfers, watcher_next_address, watcher_next_utxo, watcher_rgb_wallet};
use bitmask_core::web::{json_parse, resolve, resolve_all};
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;
use bitmask_core::web::bp::get_btc_wallet;

wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn simulate_deeadlock() -> anyhow::Result<()> {
    let keys = create_new_wallet(WalletSeed::default()).await?;

    let sk = keys.private.nostr_prv.clone();
    resolve_all(vec![
        get_btc_wallet(sk.clone()),
        watcher_next_address(sk.clone(), "bitcoin".to_string()),
        watcher_next_address(sk.clone(), "bitcoin".to_string()),
        watcher_rgb_wallet(sk.clone()),
        list_transfers(sk.clone()),
        get_btc_wallet(sk.clone()),
        watcher_next_address(sk.clone(), "rgb".to_string()),
        watcher_rgb_wallet(sk.clone()),
        get_btc_wallet(sk.clone()),
        watcher_rgb_wallet(sk.clone()),
        watcher_next_utxo(sk.clone(), "rgb".to_string()),
        get_btc_wallet(sk.clone()),
        watcher_rgb_wallet(sk.clone()),
        watcher_next_utxo(sk.clone(), "rgb".to_string()),
        get_btc_wallet(sk.clone()),
        list_transfers(sk.clone()),
        watcher_rgb_wallet(sk.clone()),
    ]).await;

    resolve(destroy_watcher(sk.clone())).await;

    Ok(())
}
