#![cfg(target_arch = "wasm32")]

use crate::rgb::web::seeder::{
    ContractSeed, SeedInvoice, WalletSeed, create_new_contract, create_new_invoice,
    create_new_wallet,
};
use amplify::none;
use bitmask_core::info;
use bitmask_core::rgb::models::ContractAmount;
use bitmask_core::structs::{
    ChainFee, ChainTarget, ContractResponse, PrivateWalletData, RgbInvoiceValue,
    RgbTransferRequest, UDAValue,
};
use bitmask_core::web::resolve;
use bitmask_core::web::rgb::create_rgb_transfer;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn create_new_rgb20_invoice() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let issue_req = ContractSeed::fungible(issuer_keys, none!());
    let issue_res = create_new_contract(issue_req).await?;
    let ContractResponse {
        contract,
        precision,
        ..
    } = issue_res[0].clone();

    let amount = ContractAmount::new(1, 0, precision);
    let rgb_amount = RgbInvoiceValue::Value(amount.to_string());

    let new_invoice = SeedInvoice::with_contract(contract, rgb_amount, owners_keys);
    let invoice_resp = create_new_invoice(new_invoice).await?;
    assert_ne!(invoice_resp.invoice, String::new());
    Ok(())
}

#[wasm_bindgen_test]
async fn create_new_rgb21_invoice() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let PrivateWalletData {
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let issue_req = ContractSeed::uda(issuer_keys, none!());
    let issue_res = create_new_contract(issue_req).await?;
    let ContractResponse { contract, .. } = issue_res[0].clone();

    let rgb_amount = RgbInvoiceValue::UDA(UDAValue::default());
    let new_invoice = SeedInvoice::with_contract(contract, rgb_amount, owners_keys);
    let invoice_resp = create_new_invoice(new_invoice).await?;
    assert_ne!(invoice_resp.invoice, String::new());

    Ok(())
}

#[wasm_bindgen_test]
async fn create_new_rgb20_transfer() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let issue_req = ContractSeed::fungible(issuer_keys.clone(), none!());
    let issue_res = create_new_contract(issue_req).await?;
    let ContractResponse {
        contract_id,
        iface,
        contract,
        precision,
        ..
    } = issue_res[0].clone();

    let amount = ContractAmount::new(1, 0, precision);
    let rgb_amount = RgbInvoiceValue::Value(amount.to_string());

    let new_invoice = SeedInvoice::with_contract(contract, rgb_amount, owners_keys);
    let invoice_resp = create_new_invoice(new_invoice).await?;
    assert_ne!(invoice_resp.invoice, String::new());

    info!(format!("Create Transfer"));
    let transfer_req = RgbTransferRequest {
        contract_id: contract_id.to_string(),
        iface: iface.to_string(),
        invoice: invoice_resp.invoice.to_string(),
        sender_amount: None,
        chain: ChainTarget::Bitcoin,
        chain_fee: ChainFee::Value(1000),
    };

    let PrivateWalletData {
        nostr_prv: ref sk, ..
    } = issuer_keys.private;

    let sender_sk = sk;
    let transfer_req = serde_wasm_bindgen::to_value(&transfer_req).expect("");
    let _: JsValue = resolve(create_rgb_transfer(sender_sk.to_string(), transfer_req)).await;

    Ok(())
}

#[wasm_bindgen_test]
async fn create_new_rgb21_transfer() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let issue_req = ContractSeed::uda(issuer_keys.clone(), none!());
    let issue_res = create_new_contract(issue_req).await?;
    let ContractResponse {
        contract_id,
        iface,
        contract,
        ..
    } = issue_res[0].clone();

    let amount = UDAValue::default();
    let rgb_amount = RgbInvoiceValue::UDA(amount);

    let new_invoice = SeedInvoice::with_contract(contract, rgb_amount, owners_keys.clone());
    let invoice_resp = create_new_invoice(new_invoice).await?;
    assert_ne!(invoice_resp.invoice, String::new());

    info!(format!("Create Transfer"));
    let transfer_req = RgbTransferRequest {
        contract_id: contract_id.to_string(),
        iface: iface.to_string(),
        invoice: invoice_resp.invoice.to_string(),
        sender_amount: None,
        chain: ChainTarget::Bitcoin,
        chain_fee: ChainFee::Value(1000),
    };

    let PrivateWalletData {
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private;

    let PrivateWalletData { .. } = owners_keys.private;

    let transfer_req = serde_wasm_bindgen::to_value(&transfer_req).expect("");
    let _: JsValue = resolve(create_rgb_transfer(sender_sk.to_string(), transfer_req)).await;

    Ok(())
}
