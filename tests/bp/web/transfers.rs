#![cfg(target_arch = "wasm32")]
use crate::bp::web::seeder::{WalletSeed, create_new_wallet};
use crate::bp::web::utils::send_coins;
use bitmask_core::info;
use bitmask_core::structs::{NextAddressResponse, NextUtxoResponse};
use bitmask_core::web::wallet::{destroy_watcher, watcher_next_address, watcher_next_utxo};
use bitmask_core::web::{json_parse, resolve};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn create_new_btc_transfer() -> anyhow::Result<()> {
    let keys = create_new_wallet(WalletSeed::default()).await?;

    let iface = "Bitcoin";
    info!("Get Address");
    let sk = &keys.private.nostr_prv;
    let next_address: JsValue =
        resolve(watcher_next_address(sk.to_string(), iface.to_string())).await;
    let next_address: NextAddressResponse = json_parse(&next_address);
    info!(format!("Show Address {}", next_address.address));

    let resp = send_coins(&next_address.address, "1").await;
    info!(format!("Issuer Receive Coin {:?}", resp));

    info!("Get UTXO");
    let next_utxo: JsValue = resolve(watcher_next_utxo(sk.to_string(), iface.to_string())).await;

    let next_utxo: NextUtxoResponse = json_parse(&next_utxo);
    info!(format!("Show UTXO {:?}", next_utxo));

    resolve(destroy_watcher(sk.to_string())).await;

    Ok(())
}
