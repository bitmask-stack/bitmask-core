#![allow(clippy::unwrap_used, clippy::expect_used)] // This is just an API, not business logic, so errors here aren't as impactful

use js_sys::Promise;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, future_to_promise};

// use crate::rgb::{
//     // issue::IssueContractOptions,
//     // transfer::{RgbAcceptOptions, RgbInvoiceOptions, RgbTransferOptions},
// };
// use crate::structs::{
//     AcceptRequest, IssueLocalResponse, IssueRequest, ListContractsResponse, ListTransfersResponse,
//     RgbAcceptLocalResponse, RgbInvoiceLocalResponse, RgbInvoiceRequest, RgbOfferLocalResponse,
//     RgbTransferLocalResponse, RgbTransferRequest,  WatcherRequest,
// };
use crate::rgb::models::ContractAmount;

use crate::structs::{SecretString, SignPsbtRequest};

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    console_error_panic_hook::set_once();
}

pub async fn resolve(promise: Promise) -> JsValue {
    JsFuture::from(promise).await.unwrap()
}

pub fn to_string(js_str: &JsValue) -> String {
    js_str.as_string().unwrap()
}

pub fn json_parse<T: DeserializeOwned>(js_str: &JsValue) -> T {
    serde_json::from_str(&js_str.as_string().unwrap()).expect("parsed json")
}

trait FromString {
    fn from_string(str: String) -> JsValue;
}

impl FromString for JsValue {
    fn from_string(str: String) -> JsValue {
        JsValue::from_str(&str)
    }
}

pub mod constants {
    use super::*;

    #[wasm_bindgen]
    pub fn get_network() -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let result = crate::constants::get_network().await;

            Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            ))
        })
    }

    #[wasm_bindgen]
    pub fn switch_network(network_str: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::constants::switch_network(&network_str).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_env(key: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let result = crate::constants::get_env(&key).await;

            Ok(JsValue::from_string(
                serde_json::to_string(&result).unwrap(),
            ))
        })
    }

    #[wasm_bindgen]
    pub fn set_env(key: String, value: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            crate::constants::set_env(&key, &value).await;

            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen]
    pub fn sleep(ms: i32) -> js_sys::Promise {
        js_sys::Promise::new(&mut |resolve, _| {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
                .unwrap();
        })
    }
}

pub mod bitcoin {
    use super::*;
    use crate::structs::PublishPsbtRequest;

    #[wasm_bindgen]
    pub fn hash_password(password: String) -> String {
        set_panic_hook();

        crate::bitcoin::hash_password(&SecretString(password))
            .unwrap()
            .to_string()
    }

    #[wasm_bindgen]
    pub fn new_mnemonic(password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::new_mnemonic(&SecretString(password)).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn save_mnemonic(seed: String, password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::save_mnemonic(&SecretString(seed), &SecretString(password)).await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn decrypt_wallet(
        hash: String,
        encrypted_descriptors: String,
        seed_password: String,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::decrypt_wallet(
                &SecretString(hash),
                &SecretString(encrypted_descriptors),
                &SecretString(seed_password),
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn upgrade_wallet(
        hash: String,
        encrypted_descriptors: String,
        seed_password: String,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::upgrade_wallet(
                &SecretString(hash),
                &SecretString(encrypted_descriptors),
                &SecretString(seed_password),
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn new_wallet(hash: String, seed_password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::new_wallet(&SecretString(hash), &SecretString(seed_password))
                .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn encrypt_wallet(mnemonic: String, hash: String, seed_password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::encrypt_wallet(
                &SecretString(mnemonic),
                &SecretString(hash),
                &SecretString(seed_password),
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_wallet_data(descriptor: String, change_descriptor: Option<String>) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            let change_descriptor = change_descriptor.map(SecretString);
            match crate::bitcoin::get_wallet_data(
                &SecretString(descriptor),
                change_descriptor.as_ref(),
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn sync_wallets() -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::sync_wallets().await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_new_address(descriptor: String, change_descriptor: Option<String>) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            let change_descriptor = change_descriptor.map(SecretString);
            match crate::bitcoin::get_new_address(
                &SecretString(descriptor),
                change_descriptor.as_ref(),
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn send_sats(
        descriptor: String,
        change_descriptor: String,
        destination: String,
        amount: u64,
        broadcast: bool,
        fee_rate: Option<f32>,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::send_sats(
                &SecretString(descriptor),
                &SecretString(change_descriptor),
                &destination,
                amount,
                broadcast,
                fee_rate,
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn fund_vault(
        descriptor: String,
        change_descriptor: String,
        rgb_address: String,
        broadcast: bool,
        fee_rate: Option<f32>,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::fund_vault(
                &SecretString(descriptor),
                &SecretString(change_descriptor),
                &rgb_address,
                broadcast,
                fee_rate,
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_assets_vault(rgb_descriptor_xpub: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bitcoin::get_assets_vault(&SecretString(rgb_descriptor_xpub)).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn drain_wallet(
        destination: String,
        descriptor: String,
        change_descriptor: Option<String>,
        fee_rate: Option<f32>,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let change_descriptor = change_descriptor.map(SecretString);

            match crate::bitcoin::drain_wallet(
                &destination,
                &SecretString(descriptor),
                change_descriptor.as_ref(),
                fee_rate,
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn bump_fee(
        txid: String,
        fee_rate: f32,
        descriptor: String,
        change_descriptor: Option<String>,
        broadcast: bool,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let change_descriptor = change_descriptor.map(SecretString);
            match crate::bitcoin::bump_fee(
                txid,
                fee_rate,
                &SecretString(descriptor),
                change_descriptor.as_ref(),
                broadcast,
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn psbt_sign_and_publish_file(request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let psbt_req: SignPsbtRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::bitcoin::sign_and_publish_psbt_file_bp(psbt_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn psbt_publish_file(request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let psbt_req: PublishPsbtRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::bitcoin::publish_psbt_file_bp(psbt_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }
}

pub mod bp {
    use super::*;
    use crate::bp::FundOptions;
    use crate::rgb::RgbError;
    use bpstd::Sats;

    #[wasm_bindgen]
    pub fn fund_rgb_vault(
        sk: String,
        fee_rate: f32,
        secrets: JsValue,
        broadcast: bool,
        fund_value: Option<u64>,
        onetime_fee: bool,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let secrets: Vec<SecretString> = serde_wasm_bindgen::from_value(secrets).unwrap();
            let options = FundOptions {
                broadcast,
                private_keys: secrets,
                fund_value: fund_value.map(Sats::from_sats),
                onetime_fee,
            };
            match crate::bp::fund_vault(&sk, fee_rate, options).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn transfer_sats(
        sk: String,
        address: String,
        sats: u64,
        fee_rate: f32,
        secrets: JsValue,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let secrets: Vec<SecretString> = serde_wasm_bindgen::from_value(secrets).unwrap();
            match crate::bp::transfer_btc(&sk, &address, sats, fee_rate, secrets).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_btc_wallet(sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::bp::get_btc_wallet(&sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn backup_btc_data(nostr_hex_sk: String) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            match crate::bp::btc_backup(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Backup(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn recover_funds_from_bad_wallets(
        seed: String,
        pass_phrase: String,
        recover_address: String,
    ) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            match crate::bp::recover::recover_funds_from_bad_wallets(
                SecretString(seed),
                SecretString(pass_phrase),
                &recover_address,
            )
            .await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn restore_btc_data(nostr_hex_sk: String) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            match crate::bp::btc_restore(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Restore(err).to_string())),
            }
        })
    }
}

pub mod wallet {
    use crate::rgb::RgbError;
    use crate::structs::WatcherRequest;
    use crate::web::{FromString, set_panic_hook};
    use js_sys::Promise;
    use wasm_bindgen::JsValue;
    use wasm_bindgen::prelude::wasm_bindgen;
    use wasm_bindgen_futures::future_to_promise;

    #[wasm_bindgen]
    pub fn create_watcher(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let mut pay_req: WatcherRequest = serde_wasm_bindgen::from_value(request).unwrap();
            pay_req.recover = false;

            match crate::wallet::watcher::create_watcher(&nostr_hex_sk, pay_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(
                    RgbError::CreateWatcher(err).to_string(),
                )),
            }
        })
    }

    #[wasm_bindgen]
    pub fn recover_watcher(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let mut pay_req: WatcherRequest = serde_wasm_bindgen::from_value(request).unwrap();
            pay_req.recover = true;

            match crate::wallet::watcher::create_watcher(&nostr_hex_sk, pay_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Recover(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn destroy_watcher(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::wallet::watcher::destroy_watcher(&nostr_hex_sk, false).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::DropWatcher(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn destroy_recover_watcher(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::wallet::watcher::destroy_watcher(&nostr_hex_sk, true).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Recover(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn check_watcher(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::wallet::watcher::check_watcher(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(
                    RgbError::CheckWatcher(err).to_string(),
                )),
            }
        })
    }

    #[wasm_bindgen]
    pub fn watcher_next_address(nostr_hex_sk: String, request: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::wallet::watcher::watcher_next_address(&nostr_hex_sk, &request, true).await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(
                    RgbError::CreateNextAddress(err).to_string(),
                )),
            }
        })
    }

    #[wasm_bindgen]
    pub fn watcher_next_utxo(nostr_hex_sk: String, request: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::wallet::watcher::watcher_next_utxo(&nostr_hex_sk, &request).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(
                    RgbError::CreateNextUtxo(err).to_string(),
                )),
            }
        })
    }

    #[wasm_bindgen]
    pub fn watcher_unspent_utxos(nostr_hex_sk: String, request: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::wallet::watcher::watcher_unspent_utxo(&nostr_hex_sk, &request, false).await
            {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(
                    RgbError::CreateUnspentUtxo(err).to_string(),
                )),
            }
        })
    }
}

pub mod rgb {

    use super::*;
    use crate::rgb::RgbError;
    use crate::rgb::contract::RetrieveContractOptions;
    use crate::rgb::issue::IssueContractOptions;
    use crate::rgb::swap::{BidOptions, OfferOptions};
    use crate::rgb::transfer::{RgbAcceptOptions, RgbInvoiceOptions, RgbTransferOptions};
    use crate::structs::{
        AcceptRequest, ChainFee, IssueRequest, RgbBidRequest, RgbCancelBidsRequest,
        RgbCancelOffersRequest, RgbInvoiceRequest, RgbInvoiceValue, RgbOfferRequest,
        RgbTransferRequest, RgbTransferResponse, SaveTransferRequest,
    };
    use serde::Deserialize;
    use wasm_bindgen::prelude::wasm_bindgen;

    fn remap_issue_request(request: JsValue) -> JsValue {
        let mut v: serde_json::Value =
            serde_wasm_bindgen::from_value(request).unwrap_or(serde_json::Value::Null);
        if let serde_json::Value::Object(map) = &mut v
            && let Some(supply) = map.get("supply")
            && !supply.is_string()
        {
            let s = match supply {
                serde_json::Value::Number(n) => n.to_string(),
                other => other.to_string().trim_matches('"').to_string(),
            };
            map.insert("supply".into(), serde_json::Value::String(s));
        }
        serde_wasm_bindgen::to_value(&v).unwrap_or(JsValue::NULL)
    }

    fn remap_full_transfer_request(request: JsValue) -> JsValue {
        let mut v: serde_json::Value =
            serde_wasm_bindgen::from_value(request).unwrap_or(serde_json::Value::Null);
        remap_transfer_json(&mut v);
        serde_wasm_bindgen::to_value(&v).unwrap_or(JsValue::NULL)
    }

    fn remap_transfer_json(v: &mut serde_json::Value) {
        if let serde_json::Value::Object(map) = v {
            if !map.contains_key("invoice")
                && let Some(inv) = map.get("rgbInvoice").cloned()
            {
                map.insert("invoice".into(), inv);
            }
            if !map.contains_key("chainFee")
                && let Some(fee) = map.get("fee").cloned()
            {
                map.insert("chainFee".into(), fee);
            }
        }
    }

    fn remap_airdrop_transfers_json(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Array(items) => {
                for item in items {
                    remap_transfer_json(item);
                }
            }
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::Array(items)) = map.get_mut("transfers") {
                    for item in items {
                        remap_transfer_json(item);
                    }
                } else {
                    if !map.contains_key("invoice")
                        && let Some(inv) = map.get("rgbInvoice").cloned()
                    {
                        map.insert("invoice".into(), inv);
                    }
                    if !map.contains_key("chainFee")
                        && let Some(fee) = map.get("fee").cloned()
                    {
                        map.insert("chainFee".into(), fee);
                    }
                }
            }
            _ => {}
        }
    }

    fn js_typed_error(value: serde_json::Value) -> String {
        serde_json::to_string(&value).unwrap_or_else(|_| r#"{"error":"encode"}"#.to_string())
    }

    fn decimal_units(s: &str) -> Option<(u128, u32)> {
        let s = s.trim().replace(['_', ','], "");
        if s.is_empty() {
            return None;
        }
        let (int, frac) = match s.as_str().split_once('.') {
            Some((i, f)) => (i, f.trim_end_matches('0')),
            None => (s.as_str(), ""),
        };
        let int = int.trim_start_matches('+');
        if int.is_empty() && frac.is_empty() {
            return None;
        }
        if !int.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let digits = format!("{int}{frac}");
        let digits = if digits.chars().all(|c| c == '0') {
            "0".to_string()
        } else {
            digits.trim_start_matches('0').to_string()
        };
        let val = digits.parse::<u128>().ok()?;
        Some((val, frac.len() as u32))
    }

    fn amount_exceeds_cap(amount: &str, cap: &str) -> Result<bool, ()> {
        let (av, ap) = decimal_units(amount).ok_or(())?;
        let (cv, cp) = decimal_units(cap).ok_or(())?;
        let p = ap.max(cp);
        let av = av.checked_mul(10u128.pow(p.saturating_sub(ap))).ok_or(())?;
        let cv = cv.checked_mul(10u128.pow(p.saturating_sub(cp))).ok_or(())?;
        Ok(av > cv)
    }

    fn invoice_value_amount(value: &RgbInvoiceValue) -> String {
        match value {
            RgbInvoiceValue::Value(v) => v.clone(),
            RgbInvoiceValue::UDA(uda) => uda.fraction.to_string(),
        }
    }

    fn apply_airdrop_fee(req: &mut RgbTransferRequest, fee: u64) {
        match req.chain_fee {
            ChainFee::None | ChainFee::Value(0) => {
                req.chain_fee = ChainFee::Value(fee);
            }
            _ => {}
        }
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum AirdropRequestBody {
        List(Vec<RgbTransferRequest>),
        Wrapped(AirdropTransfers),
        One(RgbTransferRequest),
    }

    #[derive(Deserialize)]
    struct AirdropTransfers {
        transfers: Vec<RgbTransferRequest>,
        #[serde(default, alias = "max_claim", rename = "maxClaim")]
        max_claim: Option<String>,
    }

    fn parse_airdrop_transfers(
        mut raw: serde_json::Value,
    ) -> Result<(Vec<RgbTransferRequest>, Option<String>), String> {
        remap_airdrop_transfers_json(&mut raw);
        match serde_json::from_value::<AirdropRequestBody>(raw) {
            Ok(AirdropRequestBody::List(transfers)) => Ok((transfers, None)),
            Ok(AirdropRequestBody::Wrapped(body)) => Ok((body.transfers, body.max_claim)),
            Ok(AirdropRequestBody::One(one)) => Ok((vec![one], None)),
            Err(err) => Err(js_typed_error(serde_json::json!({
                "error": "parse",
                "message": format!("create_airdrop request must be transfers: {err}"),
            }))),
        }
    }

    fn rgb011_removed(name: &'static str) -> Promise {
        future_to_promise(async move {
            Err(JsValue::from_string(format!(
                "{name} is not on RGB Protocol 0.11.1-rc.11"
            )))
        })
    }

    #[wasm_bindgen]
    pub fn get_rgb_version() -> String {
        set_panic_hook();

        crate::rgb::get_rgb_version()
    }

    #[wasm_bindgen]
    pub fn get_rgb_wallet(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::get_wallet_data(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Account(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_contract(nostr_hex_sk: String, request: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::get_contract_media(&nostr_hex_sk, request).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Account(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn list_contracts(nostr_hex_sk: String, _: bool) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::list_contracts(&nostr_hex_sk, true).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Account(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn import_contract(nostr_hex_sk: String, request: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::import_contract(&nostr_hex_sk, request).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Account(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn issue_contract(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: IssueRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let options = IssueContractOptions::default();

            match crate::rgb::issue_contract(&nostr_hex_sk, req, options).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Issue(err).to_string())),
            }
        })
    }

    /// TypeScript `BMC.full_issue_contract`. Same live 0.11 issue path as `issue_contract`.
    #[wasm_bindgen]
    pub fn full_issue_contract(nostr_hex_sk: String, request: JsValue) -> Promise {
        issue_contract(nostr_hex_sk, remap_issue_request(request))
    }

    #[wasm_bindgen]
    pub fn issue_contract_proxy(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: IssueRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let options = IssueContractOptions {
                proxy: true,
                ..IssueContractOptions::default()
            };

            match crate::rgb::issue_contract(&nostr_hex_sk, req, options).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Issue(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_rgb_invoice(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbInvoiceRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let options = RgbInvoiceOptions::default();

            match crate::rgb::create_invoice(&nostr_hex_sk, req, options).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Invoice(err).to_string())),
            }
        })
    }

    /// TypeScript `BMC.rgb_create_invoice`.
    #[wasm_bindgen]
    pub fn rgb_create_invoice(nostr_hex_sk: String, request: JsValue) -> Promise {
        create_rgb_invoice(nostr_hex_sk, request)
    }

    #[wasm_bindgen]
    pub fn remove_rgb_invoice(nostr_hex_sk: String, request: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let options = RetrieveContractOptions::default();
            match crate::rgb::remove_invoice(&nostr_hex_sk, request, options).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Invoice(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn list_rgb_invoices(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let options = RetrieveContractOptions::default();
            match crate::rgb::list_invoices(&nostr_hex_sk, options).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Invoice(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn decode_rgb_invoice(invoice: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::decode_invoice(invoice).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Invoice(err).to_string())),
            }
        })
    }

    /// TypeScript `BMC.decode_invoice`.
    #[wasm_bindgen]
    pub fn decode_invoice(invoice: String) -> Promise {
        decode_rgb_invoice(invoice)
    }

    #[wasm_bindgen]
    pub fn can_create_transfer_contract(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbTransferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let options = RgbTransferOptions::default();

            match crate::rgb::can_transfer_contract(&nostr_hex_sk, req, options).await {
                Ok(resp) => Ok(JsValue::from_string(serde_json::to_string(&resp).unwrap())),
                Err(err) => Err(JsValue::from_string(
                    RgbError::TransferValidation(err.to_string()).to_string(),
                )),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_rgb_transfer(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbTransferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let options = RgbTransferOptions::default();

            match crate::rgb::transfer_contract(&nostr_hex_sk, req, options).await {
                Ok(resp) => Ok(JsValue::from_string(serde_json::to_string(&resp).unwrap())),
                Err(err) => Err(JsValue::from_string(RgbError::Transfer(err).to_string())),
            }
        })
    }

    /// TypeScript `BMC.transfer_asset`. Live 0.11 `transfer_contract` / `create_rgb_transfer`.
    #[wasm_bindgen]
    pub fn transfer_asset(nostr_hex_sk: String, request: JsValue) -> Promise {
        create_rgb_transfer(nostr_hex_sk, remap_full_transfer_request(request))
    }

    /// TypeScript `BMC.full_transfer_asset`. Same live 0.11 transfer path.
    #[wasm_bindgen]
    pub fn full_transfer_asset(nostr_hex_sk: String, request: JsValue) -> Promise {
        create_rgb_transfer(nostr_hex_sk, remap_full_transfer_request(request))
    }

    #[wasm_bindgen]
    pub fn create_and_publish_rgb_transfer(
        nostr_hex_sk: String,
        request: JsValue,
        secrets: JsValue,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbTransferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let secrets: Vec<SecretString> = serde_wasm_bindgen::from_value(secrets).unwrap();
            let options = RgbTransferOptions::broadcast(secrets);

            match crate::rgb::transfer_contract(&nostr_hex_sk, req, options).await {
                Ok(resp) => Ok(JsValue::from_string(serde_json::to_string(&resp).unwrap())),
                Err(err) => Err(JsValue::from_string(RgbError::Transfer(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn psbt_sign_file(_nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let psbt_req: SignPsbtRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::bitcoin::sign_psbt_file(psbt_req).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn accept_transfer(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: AcceptRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let options = RgbAcceptOptions::default();

            match crate::rgb::accept_transfer(&nostr_hex_sk, req, options).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Accept(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn save_transfer(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: SaveTransferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::save_transfer(&nostr_hex_sk, req, default!()).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Accept(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn list_transfers(nostr_hex_sk: String) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            match crate::rgb::list_transfers(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Account(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn verify_transfers(nostr_hex_sk: String) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            match crate::rgb::verify_transfers(&nostr_hex_sk, true).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(
                    RgbError::CheckTransfer(err).to_string(),
                )),
            }
        })
    }

    #[wasm_bindgen]
    pub fn list_offers(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::public_offers(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Swap(err).to_string())),
            }
        })
    }

    /// TypeScript `BMC.public_offers`.
    #[wasm_bindgen]
    pub fn public_offers(nostr_hex_sk: String) -> Promise {
        list_offers(nostr_hex_sk)
    }

    #[wasm_bindgen]
    pub fn offers(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::offers(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Swap(err).to_string())),
            }
        })
    }

    /// TypeScript `BMC.my_offers`.
    #[wasm_bindgen]
    pub fn my_offers(nostr_hex_sk: String) -> Promise {
        offers(nostr_hex_sk)
    }

    /// TypeScript `BMC.my_orders` (offers only; bids are `my_bids`).
    #[wasm_bindgen]
    pub fn my_orders(nostr_hex_sk: String) -> Promise {
        offers(nostr_hex_sk)
    }

    #[wasm_bindgen]
    pub fn bids(nostr_hex_sk: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::bids(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Swap(err).to_string())),
            }
        })
    }

    /// TypeScript `BMC.my_bids`.
    #[wasm_bindgen]
    pub fn my_bids(nostr_hex_sk: String) -> Promise {
        bids(nostr_hex_sk)
    }

    #[wasm_bindgen]
    pub fn offer_swap_suppl(nostr_hex_sk: String, offer_id: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::offer_swap_suppl(&nostr_hex_sk, &offer_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Swap(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn bid_swap_suppl(nostr_hex_sk: String, bid_id: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::rgb::bid_swap_suppl(&nostr_hex_sk, &bid_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Swap(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn can_create_offer(nostr_hex_sk: String, request: JsValue, _secrets: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbOfferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let options = OfferOptions::default();

            match crate::rgb::can_create_offers(&nostr_hex_sk, req, options).await {
                Ok(resp) => Ok(JsValue::from_string(serde_json::to_string(&resp).unwrap())),
                Err(err) => Err(JsValue::from_string(
                    RgbError::SwapValidation(err.to_string()).to_string(),
                )),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_offer(nostr_hex_sk: String, request: JsValue, secrets: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbOfferRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let keys: Vec<SecretString> = serde_wasm_bindgen::from_value(secrets).unwrap();
            let options = OfferOptions::p2p(keys);

            match crate::rgb::create_offer(&nostr_hex_sk, req, options).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Swap(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn cancel_offer(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbCancelOffersRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::cancel_offer(&nostr_hex_sk, req).await {
                Ok(resp) => Ok(JsValue::from_string(serde_json::to_string(&resp).unwrap())),
                Err(err) => Err(JsValue::from_string(RgbError::Swap(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn can_create_bid(nostr_hex_sk: String, request: JsValue, _secrets: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbBidRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let options = BidOptions::default();

            match crate::rgb::can_create_bid(&nostr_hex_sk, req, options).await {
                Ok(resp) => Ok(JsValue::from_string(serde_json::to_string(&resp).unwrap())),
                Err(err) => Err(JsValue::from_string(
                    RgbError::SwapValidation(err.to_string()).to_string(),
                )),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_bid(nostr_hex_sk: String, request: JsValue, secrets: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbBidRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let keys: Vec<SecretString> = serde_wasm_bindgen::from_value(secrets).unwrap();
            let options = BidOptions::p2p(keys);

            match crate::rgb::create_bid(&nostr_hex_sk, req, options).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Swap(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn cancel_bid(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: RgbCancelBidsRequest = serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::cancel_bid(&nostr_hex_sk, req).await {
                Ok(resp) => Ok(JsValue::from_string(serde_json::to_string(&resp).unwrap())),
                Err(err) => Err(JsValue::from_string(RgbError::Swap(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_consignment(consig_or_receipt_id: String) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            match crate::rgb::proxy::get_consignment(&consig_or_receipt_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_media_metadata(media_id: String) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            match crate::rgb::proxy::get_media_metadata(&media_id).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn import_consignments(request: JsValue) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            let map: std::collections::BTreeMap<String, String> =
                serde_wasm_bindgen::from_value(request).unwrap();
            match crate::rgb::proxy::post_consignments(map, None).await {
                Ok(()) => Ok(JsValue::from_string("true".to_string())),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn create_psbt(_nostr_hex_sk: String, _request: JsValue) -> Promise {
        rgb011_removed("create_psbt")
    }

    #[wasm_bindgen]
    pub fn hidden_contract(_nostr_hex_sk: String, _contract_id: String) -> Promise {
        rgb011_removed("hidden_contract")
    }

    #[wasm_bindgen]
    pub fn list_interfaces(_nostr_hex_sk: String) -> Promise {
        rgb011_removed("list_interfaces")
    }

    #[wasm_bindgen]
    pub fn list_schemas(_nostr_hex_sk: String) -> Promise {
        rgb011_removed("list_schemas")
    }

    #[wasm_bindgen]
    pub fn watcher_details(_nostr_hex_sk: String, _name: String) -> Promise {
        rgb011_removed("watcher_details")
    }

    #[wasm_bindgen]
    pub fn watcher_address(_nostr_hex_sk: String, _name: String, _address: String) -> Promise {
        rgb011_removed("watcher_address")
    }

    #[wasm_bindgen]
    pub fn watcher_utxo(_nostr_hex_sk: String, _name: String, _utxo: String) -> Promise {
        rgb011_removed("watcher_utxo")
    }

    #[wasm_bindgen]
    pub fn remove_transfer(_nostr_hex_sk: String, _request: JsValue) -> Promise {
        rgb011_removed("remove_transfer")
    }

    #[wasm_bindgen]
    pub fn create_swap(_nostr_hex_sk: String, _request: JsValue) -> Promise {
        rgb011_removed("create_swap")
    }

    #[wasm_bindgen]
    pub fn create_auction_offers(_nostr_hex_sk: String, _request: JsValue) -> Promise {
        rgb011_removed("create_auction_offers")
    }

    #[wasm_bindgen]
    pub fn create_auction_bid(_nostr_hex_sk: String, _request: JsValue) -> Promise {
        rgb011_removed("create_auction_bid")
    }

    #[wasm_bindgen]
    pub fn finish_auction_offers(_nostr_hex_sk: String, _request: JsValue) -> Promise {
        rgb011_removed("finish_auction_offers")
    }

    #[wasm_bindgen]
    pub fn list_auctions() -> Promise {
        rgb011_removed("list_auctions")
    }

    #[wasm_bindgen]
    pub fn direct_swap(_nostr_hex_sk: String, _request: JsValue) -> Promise {
        rgb011_removed("direct_swap")
    }

    #[wasm_bindgen]
    pub fn import_uda_data(_request: JsValue) -> Promise {
        rgb011_removed("import_uda_data")
    }

    #[wasm_bindgen]
    pub fn create_airdrop(
        nostr_hex_sk: String,
        request: JsValue,
        secrets: JsValue,
        fee: u64,
        max_claim: String,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let raw: serde_json::Value =
                serde_wasm_bindgen::from_value(request).unwrap_or(serde_json::Value::Null);
            let (mut transfers, wrapped_cap) = match parse_airdrop_transfers(raw) {
                Ok(parsed) => parsed,
                Err(err) => return Err(JsValue::from_string(err)),
            };
            let secrets: Vec<SecretString> = match serde_wasm_bindgen::from_value(secrets) {
                Ok(keys) => keys,
                Err(err) => {
                    return Err(JsValue::from_string(js_typed_error(serde_json::json!({
                        "error": "parse",
                        "message": format!("create_airdrop secrets: {err}"),
                    }))));
                }
            };
            let cap = if !max_claim.is_empty() {
                max_claim
            } else {
                wrapped_cap.unwrap_or_default()
            };

            let mut responses: Vec<RgbTransferResponse> = Vec::new();
            for req in transfers.iter_mut() {
                apply_airdrop_fee(req, fee);

                if !cap.is_empty() {
                    let mut amount = match &req.sender_amount {
                        Some(value) => invoice_value_amount(value),
                        None => String::new(),
                    };
                    if (amount.is_empty() || amount == "0")
                        && let Ok(decoded) = crate::rgb::decode_invoice(req.invoice.clone()).await
                    {
                        amount = invoice_value_amount(&decoded.amount);
                    }
                    match amount_exceeds_cap(&amount, &cap) {
                        Ok(true) => {
                            return Err(JsValue::from_string(js_typed_error(serde_json::json!({
                                "error": "max_claim",
                                "message": "transfer amount exceeds max_claim",
                                "amount": amount,
                                "maxClaim": cap,
                                "invoice": req.invoice,
                            }))));
                        }
                        Ok(false) => {}
                        Err(()) => {
                            return Err(JsValue::from_string(js_typed_error(serde_json::json!({
                                "error": "max_claim",
                                "message": "could not compare invoice or sender amount to max_claim",
                                "amount": amount,
                                "maxClaim": cap,
                                "invoice": req.invoice,
                            }))));
                        }
                    }
                }

                let options = RgbTransferOptions::broadcast(secrets.clone());
                match crate::rgb::transfer_contract(&nostr_hex_sk, req.clone(), options).await {
                    Ok(resp) => responses.push(resp),
                    Err(err) => {
                        return Err(JsValue::from_string(RgbError::Transfer(err).to_string()));
                    }
                }
            }

            Ok(JsValue::from_string(
                serde_json::to_string(&responses).unwrap(),
            ))
        })
    }

    #[wasm_bindgen]
    pub fn create_airdrop_claim(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let mut raw: serde_json::Value =
                serde_wasm_bindgen::from_value(request).unwrap_or(serde_json::Value::Null);
            remap_transfer_json(&mut raw);
            let secrets = raw
                .as_object()
                .and_then(|map| map.get("secrets"))
                .cloned()
                .and_then(|v| serde_json::from_value::<Vec<SecretString>>(v).ok());
            if let Some(map) = raw.as_object_mut() {
                map.remove("secrets");
            }
            let req: RgbTransferRequest = match serde_json::from_value(raw) {
                Ok(req) => req,
                Err(err) => {
                    return Err(JsValue::from_string(js_typed_error(serde_json::json!({
                        "error": "parse",
                        "message": format!("create_airdrop_claim request: {err}"),
                    }))));
                }
            };
            let options = match secrets {
                Some(keys) if !keys.is_empty() => RgbTransferOptions::broadcast(keys),
                _ => RgbTransferOptions::default(),
            };

            match crate::rgb::transfer_contract(&nostr_hex_sk, req, options).await {
                Ok(resp) => Ok(JsValue::from_string(serde_json::to_string(&resp).unwrap())),
                Err(err) => Err(JsValue::from_string(RgbError::Transfer(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn close_airdrop(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let _ = nostr_hex_sk;
            let body: serde_json::Value = match serde_wasm_bindgen::from_value(request) {
                Ok(value) => value,
                Err(err) => {
                    return Err(JsValue::from_string(js_typed_error(serde_json::json!({
                        "error": "parse",
                        "message": format!("close_airdrop request: {err}"),
                    }))));
                }
            };
            if body.is_null() {
                return Err(JsValue::from_string(js_typed_error(serde_json::json!({
                    "error": "parse",
                    "message": "close_airdrop request is required",
                }))));
            }

            let closed = match body {
                serde_json::Value::Object(mut map) => {
                    map.insert("closed".into(), serde_json::Value::Bool(true));
                    serde_json::Value::Object(map)
                }
                serde_json::Value::String(bundle) => serde_json::json!({
                    "closed": true,
                    "bundle": bundle,
                }),
                other => serde_json::json!({
                    "closed": true,
                    "request": other,
                }),
            };

            Ok(JsValue::from_string(
                serde_json::to_string(&closed).unwrap(),
            ))
        })
    }

    #[wasm_bindgen]
    pub fn backup_rgb_data(nostr_hex_sk: String) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            match crate::rgb::rgb_backup(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Backup(err).to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn restore_rgb_data(nostr_hex_sk: String) -> Promise {
        set_panic_hook();
        future_to_promise(async move {
            match crate::rgb::rgb_restore(&nostr_hex_sk).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(RgbError::Restore(err).to_string())),
            }
        })
    }
}

pub mod lightning {
    use super::*;

    #[wasm_bindgen]
    pub fn create_wallet(username: String, password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::create_wallet(&username, &password).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn auth(username: String, password: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::auth(&username, &password).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn ln_create_invoice(description: String, amount: u32, token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::create_invoice(&description, amount, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_balance(token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::get_balance(&token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn get_txs(token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::get_txs(&token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn pay_invoice(payment_request: String, token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::pay_invoice(&payment_request, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn check_payment(payment_hash: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::check_payment(&payment_hash).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn swap_btc_ln(token: String, ln_address: Option<String>) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::swap_btc_ln(&token, ln_address).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn swap_ln_btc(address: String, amount: u64, token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::lightning::swap_ln_btc(&address, amount, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }
}

pub mod carbonado {
    use super::*;

    #[wasm_bindgen]
    pub fn store(
        secret_key: String,
        name: String,
        data: Vec<u8>,
        force: bool,
        metadata: Option<Vec<u8>>,
    ) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::carbonado::store(&secret_key, &name, &data, force, metadata).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn retrieve(secret_key: String, name: String) -> Promise {
        set_panic_hook();

        use js_sys::Uint8Array;

        future_to_promise(async move {
            match crate::carbonado::retrieve(&secret_key, &name, vec![]).await {
                Ok((result, _)) => {
                    let array = Uint8Array::new_with_length(result.len() as u32);
                    array.copy_from(&result);
                    Ok(JsValue::from(array))
                }
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn retrieve_metadata(secret_key: String, name: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::carbonado::retrieve_metadata(&secret_key, &name).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn encode_hex(bytes: Vec<u8>) -> String {
        set_panic_hook();

        crate::carbonado::util::encode_hex(&bytes)
    }

    #[wasm_bindgen]
    pub fn encode_base64(bytes: Vec<u8>) -> String {
        set_panic_hook();

        crate::carbonado::util::encode_base64(&bytes)
    }

    #[wasm_bindgen]
    pub fn decode_hex(string: String) -> Result<Vec<u8>, JsError> {
        set_panic_hook();

        crate::carbonado::util::decode_hex(&string).map_err(|err| JsError::new(&err.to_string()))
    }

    #[wasm_bindgen]
    pub fn decode_base64(string: String) -> Result<Vec<u8>, JsError> {
        set_panic_hook();

        crate::carbonado::util::decode_base64(&string).map_err(|err| JsError::new(&err.to_string()))
    }
}

pub mod nostr {
    use super::*;

    #[wasm_bindgen]
    pub fn new_nostr_pubkey(pubkey: String, token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::nostr::new_nostr_pubkey(&pubkey, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }

    #[wasm_bindgen]
    pub fn update_nostr_pubkey(pubkey: String, token: String) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            match crate::nostr::update_nostr_pubkey(&pubkey, &token).await {
                Ok(result) => Ok(JsValue::from_string(
                    serde_json::to_string(&result).unwrap(),
                )),
                Err(err) => Err(JsValue::from_string(err.to_string())),
            }
        })
    }
}

pub mod utils {
    use crate::util::built_info;

    use super::*;

    #[wasm_bindgen]
    pub fn convert_contract_amount_raw(decimal: String, precision: u8) -> String {
        set_panic_hook();
        ContractAmount::parse(decimal, precision)
            .to_value()
            .to_string()
    }

    #[wasm_bindgen]
    pub fn convert_contract_amount_string(amount: u64, precision: u8) -> String {
        set_panic_hook();
        ContractAmount::with(amount, precision).to_string()
    }

    #[wasm_bindgen]
    pub fn parse_contract_amount(amount: String) -> JsValue {
        set_panic_hook();
        JsValue::from_string(serde_json::to_string(&ContractAmount::convert(amount)).unwrap())
    }

    #[wasm_bindgen]
    pub fn parse_contract_amount_precision(amount: String, precision: u8) -> String {
        set_panic_hook();
        ContractAmount::parse(amount, precision).to_string()
    }

    /// TypeScript `BMC.contract_amount`.
    #[wasm_bindgen]
    pub fn contract_amount(amount: u64, precision: u8) -> String {
        set_panic_hook();
        ContractAmount::with(amount, precision)
            .to_value()
            .to_string()
    }

    /// TypeScript `BMC.contract_amount_str`.
    #[wasm_bindgen]
    pub fn contract_amount_str(amount: u64, precision: u8) -> String {
        convert_contract_amount_string(amount, precision)
    }

    /// TypeScript `BMC.contract_amount_parse_str`.
    #[wasm_bindgen]
    pub fn contract_amount_parse_str(amount: String, precision: u8) -> String {
        parse_contract_amount_precision(amount, precision)
    }

    /// TypeScript `BMC.contract_amount_parse_value`.
    #[wasm_bindgen]
    pub fn contract_amount_parse_value(amount: String, precision: u8) -> String {
        convert_contract_amount_raw(amount, precision)
    }

    /// TypeScript `BMC.contract_amount_parse_decimal_value`.
    #[wasm_bindgen]
    pub fn contract_amount_parse_decimal_value(amount: String) -> JsValue {
        parse_contract_amount(amount)
    }

    #[wasm_bindgen]
    pub fn version() -> JsValue {
        set_panic_hook();

        JsValue::from_string(format!(
            "{} {} ({})",
            built_info::PKG_NAME,
            built_info::PKG_VERSION,
            built_info::BUILT_TIME_UTC,
        ))
    }
}
