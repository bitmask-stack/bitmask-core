#![allow(warnings)]
#![allow(clippy::unwrap_used, clippy::expect_used)] // This is just an API, not business logic, so errors here aren't as impactful

use crate::rgb::swap::{BidOptions, OfferOptions};
use crate::structs::{RgbBidRequest, RgbOfferRequest};
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
        AcceptRequest, IssueRequest, RgbBidRequest, RgbCancelBidsRequest, RgbCancelOffersRequest,
        RgbInvoiceRequest, RgbOfferRequest, RgbTransferRequest, SaveTransferRequest,
        WatcherRequest,
    };
    use wasm_bindgen::prelude::wasm_bindgen;

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

    use super::*;

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

    #[wasm_bindgen]
    pub fn issue_contract_proxy(nostr_hex_sk: String, request: JsValue) -> Promise {
        set_panic_hook();

        future_to_promise(async move {
            let req: IssueRequest = serde_wasm_bindgen::from_value(request).unwrap();
            let mut options = IssueContractOptions::default();
            options.proxy = true;

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
    pub fn can_create_offer(nostr_hex_sk: String, request: JsValue, secrets: JsValue) -> Promise {
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
    pub fn can_create_bid(nostr_hex_sk: String, request: JsValue, secrets: JsValue) -> Promise {
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
    pub fn create_airdrop(
        nostr_hex_sk: String,
        request: JsValue,
        secrets: JsValue,
        fee: u64,
        max_claim: String,
    ) -> Promise {
        todo!()
    }

    #[wasm_bindgen]
    pub fn create_airdrop_claim(nostr_hex_sk: String, request: JsValue) -> Promise {
        todo!()
    }

    #[wasm_bindgen]
    pub fn close_airdrop(nostr_hex_sk: String, request: JsValue) -> Promise {
        todo!()
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
