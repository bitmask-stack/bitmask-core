#![cfg(target_arch = "wasm32")]
use crate::rgb::web::utils::send_coins;
use bitmask_core::info;
use bitmask_core::structs::{
    DecryptedWalletData, NextAddressResponse, NextUtxoResponse, WatcherRequest, WatcherResponse,
};
use bitmask_core::web::bitcoin::new_mnemonic;
use bitmask_core::web::wallet::{
    create_watcher, destroy_watcher, watcher_next_address, watcher_next_utxo,
};
use bitmask_core::web::{json_parse, resolve};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn create_new_watcher() -> anyhow::Result<()> {
    info!("Get Keys");
    let issuer_keys = resolve(new_mnemonic("".to_string())).await;
    let issuer_keys: DecryptedWalletData = json_parse(&issuer_keys);

    info!("Create Watcher");
    let iface = "RGB20";
    let watcher_name = "default";

    let sk = &issuer_keys.private.nostr_prv;
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        btc_xpub: issuer_keys.public.btc_descriptor_xpub.clone(),
        rgb_xpub: issuer_keys.public.rgb_descriptor_xpub.clone(),
        recover: false,
    };

    let create_watch_req =
        serde_wasm_bindgen::to_value(&create_watch_req).expect("failed to serialize");

    let watcher_resp: JsValue =
        resolve(create_watcher(sk.to_string(), create_watch_req.clone())).await;
    let _: WatcherResponse = json_parse(&watcher_resp);

    info!("Get Address");
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
