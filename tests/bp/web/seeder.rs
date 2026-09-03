#![allow(dead_code)]
#![cfg(target_arch = "wasm32")]

use crate::bp::web::utils::send_coins;
use bitmask_core::structs::{
    DecryptedWalletData, FundVaultDetails, NextAddressResponse, PrivateWalletData,
    PublicWalletData, SecretString, WatcherRequest, WatcherResponse,
};
use bitmask_core::web::bitcoin::new_mnemonic;
use bitmask_core::web::bp::fund_rgb_vault;
use bitmask_core::web::wallet::{create_watcher, watcher_next_address};
use bitmask_core::web::{json_parse, resolve};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test_configure;

wasm_bindgen_test_configure!(run_in_browser);

#[derive(Default)]
pub struct UtxoFilter {
    pub outpoint_equal: Option<String>,
    pub amount_less_than: Option<u64>,
    pub amount_equal_than: Option<u64>,
}

impl UtxoFilter {
    pub fn amount_eq(amount: u64) -> Self {
        Self {
            outpoint_equal: None,
            amount_less_than: None,
            amount_equal_than: Some(amount),
        }
    }

    pub fn amount_lt(amount: u64) -> Self {
        Self {
            outpoint_equal: None,
            amount_less_than: Some(amount),
            amount_equal_than: None,
        }
    }
}

#[derive(Default)]
pub struct WalletSeed {
    pub amount: Option<String>,
    pub mnemonic: Option<String>,
}

pub async fn create_new_wallet(seed: WalletSeed) -> Result<DecryptedWalletData, anyhow::Error> {
    let keys = resolve(new_mnemonic("".to_string())).await;
    let keys: DecryptedWalletData = json_parse(&keys);

    let iface = "RGB20";
    let watcher_name = "default";

    let PrivateWalletData {
        nostr_prv: ref sk, ..
    } = keys.private.clone();
    let PublicWalletData {
        ref rgb_descriptor_xpub,
        ref btc_descriptor_xpub,
        ..
    } = keys.public.clone();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        btc_xpub: btc_descriptor_xpub.to_string(),
        rgb_xpub: rgb_descriptor_xpub.to_string(),
        recover: false,
    };

    let create_watch_req =
        serde_wasm_bindgen::to_value(&create_watch_req).expect("failed to serialize");

    let watcher_resp: JsValue =
        resolve(create_watcher(sk.to_string(), create_watch_req.clone())).await;
    let _: WatcherResponse = json_parse(&watcher_resp);

    let next_address: JsValue =
        resolve(watcher_next_address(sk.to_string(), iface.to_string())).await;
    let next_address: NextAddressResponse = json_parse(&next_address);

    let amount = seed.amount.map_or("1".to_string(), |x| x.to_string());
    let _ = send_coins(&next_address.address, &amount).await;

    Ok(keys)
}

pub async fn create_new_fund_wallet(
    seed: WalletSeed,
) -> Result<DecryptedWalletData, anyhow::Error> {
    let keys = resolve(new_mnemonic("".to_string())).await;
    let keys: DecryptedWalletData = json_parse(&keys);

    let btc = "Bitcoin";
    let rgb = "RGB20";
    let watcher_name = "default";

    let PrivateWalletData {
        nostr_prv: ref sk,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        ..
    } = keys.private.clone();

    let PublicWalletData {
        ref rgb_descriptor_xpub,
        ref btc_descriptor_xpub,
        ..
    } = keys.public.clone();

    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        btc_xpub: btc_descriptor_xpub.to_string(),
        rgb_xpub: rgb_descriptor_xpub.to_string(),
        recover: false,
    };

    let create_watch_req =
        serde_wasm_bindgen::to_value(&create_watch_req).expect("failed to serialize");

    let watcher_resp: JsValue =
        resolve(create_watcher(sk.to_string(), create_watch_req.clone())).await;
    let _: WatcherResponse = json_parse(&watcher_resp);

    let btc_next_address: JsValue =
        resolve(watcher_next_address(sk.to_string(), btc.to_string())).await;
    let next_address: NextAddressResponse = json_parse(&btc_next_address);

    let amount = seed.amount.map_or("1".to_string(), |x| x.to_string());
    let _ = send_coins(&next_address.address, &amount).await;

    let secrets = [
        SecretString(btc_descriptor_xprv.to_string()),
        SecretString(btc_change_descriptor_xprv.to_string()),
    ];
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid request");

    let resp = resolve(fund_rgb_vault(
        sk.clone(),
        1.0,
        secrets,
        true,
        Some(100),
        true,
    ))
    .await;
    let _: FundVaultDetails = json_parse(&resp);

    Ok(keys)
}
