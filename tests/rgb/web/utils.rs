#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(target_arch = "wasm32")]
use bitmask_core::constants::BITMASK_ENDPOINT;
use gloo_net::http::Request;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::console;

pub const USER_PUBKEY: &str = "tpubDDbo51VYPBPPjoGLX4rzBbW21qV1D9pUYJA6c5U7r3raJCuzzGqk1dUocNZ98VmxmQK5mpjZxFwc7gsyavw75v6BYbCJ6gcks2iXq83cr59";

pub const ISSUER_MNEMONIC: &str =
    "ordinary crucial edit settle pencil lion appear unlock left fly century license";

pub const OWNER_MNEMONIC: &str =
    "apology pull visa moon retreat spell elite extend secret region fly diary";

pub async fn new_block() -> Result<JsValue, JsValue> {
    let bitmask_endpoint = BITMASK_ENDPOINT.read().await.to_string();
    let endpoint = format!("{bitmask_endpoint}/regtest/block");

    let request = Request::get(&endpoint)
        .header("Content-Type", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .build();

    let request = match request {
        Ok(request) => request,
        Err(e) => return Err(JsValue::from(e.to_string())),
    };

    let response = request.send().await;
    match response {
        Ok(response) => {
            let status_code = response.status();
            if status_code == 200 {
                match response.text().await {
                    Ok(text) => Ok(JsValue::from(&text)),
                    Err(e) => Err(JsValue::from(e.to_string())),
                }
            } else {
                Err(JsValue::from(status_code))
            }
        }
        Err(e) => Err(JsValue::from(e.to_string())),
    }
}

pub async fn send_coins(address: &str, amount: &str) -> Result<JsValue, JsValue> {
    let bitmask_endpoint = BITMASK_ENDPOINT.read().await.to_string();
    let endpoint = format!("{bitmask_endpoint}/regtest/send/{address}/{amount}");

    let request = Request::get(&endpoint)
        .header("Content-Type", "application/octet-stream")
        .header("Cache-Control", "no-cache")
        .build();

    let request = match request {
        Ok(request) => request,
        Err(e) => return Err(JsValue::from(e.to_string())),
    };

    let response = request.send().await;
    match response {
        Ok(response) => {
            let status_code = response.status();
            if status_code == 200 {
                match response.text().await {
                    Ok(text) => Ok(JsValue::from(&text)),
                    Err(e) => Err(JsValue::from(e.to_string())),
                }
            } else {
                Err(JsValue::from(status_code))
            }
        }
        Err(e) => Err(JsValue::from(e.to_string())),
    }
}
