#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(target_arch = "wasm32")]

use crate::rgb::web::seeder::{
    ContractSeed, SeedInvoice, SeedTransfer, WalletSeed, create_and_publish_transfer,
    create_new_contract, create_new_invoice, create_new_transfer, create_new_wallet,
};
use crate::rgb::web::utils::new_block;
use amplify::none;
use bitmask_core::rgb::models::{AssetType, ContractAmount};
use bitmask_core::structs::{
    AcceptRequest, AcceptResponse, ChainFee, ChainTarget, ContractResponse, ContractValue,
    PrivateWalletData, PublicWalletData, PublishedPsbtResponse, RgbInvoiceValue, RgbTransferItem,
    RgbTransferRequest, RgbTransferResponse, SaveTransferRequest, SecretString, SignPsbtRequest,
    UDAValue,
};
use bitmask_core::web::bitcoin::psbt_sign_and_publish_file;
use bitmask_core::web::rgb::{
    accept_transfer, create_rgb_transfer, get_contract, list_contracts, save_transfer,
    verify_transfers,
};
use bitmask_core::web::{json_parse, resolve};
use bitmask_core::{debug, info};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn accept_new_rgb20_transfer() -> anyhow::Result<()> {
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
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref owner_sk,
        ..
    } = owners_keys.private;

    let transfer_req = serde_wasm_bindgen::to_value(&transfer_req).expect("");

    let transfer_resp: JsValue =
        resolve(create_rgb_transfer(sender_sk.to_string(), transfer_req)).await;
    let transfer_resp: RgbTransferResponse = json_parse(&transfer_resp);

    let descriptors = vec![
        SecretString(btc_descriptor_xprv.clone()),
        SecretString(btc_change_descriptor_xprv.clone()),
        SecretString(rgb_descriptor_xprv.clone()),
    ];

    let psbt_req = SignPsbtRequest {
        psbt: transfer_resp.psbt,
        descriptors,
    };
    let psbt_req = serde_wasm_bindgen::to_value(&psbt_req).expect("");
    resolve(psbt_sign_and_publish_file(psbt_req)).await;

    let save_transfer_req = SaveTransferRequest {
        consignment: transfer_resp.consig.to_string(),
    };
    let save_transfer_req = serde_wasm_bindgen::to_value(&save_transfer_req).expect("");

    let save_transfer_resp: JsValue =
        resolve(save_transfer(owner_sk.to_string(), save_transfer_req)).await;
    let save_transfer_resp: RgbTransferItem = json_parse(&save_transfer_resp);

    let _ = new_block().await;
    resolve(verify_transfers(owner_sk.to_string())).await;

    let contract_resp = resolve(get_contract(
        owner_sk.to_string(),
        save_transfer_resp.contract_id,
    ))
    .await;

    let contract_resp: ContractResponse = json_parse(&contract_resp);

    debug!(format!("Contract Data"));
    debug!(format!(
        "{} @ {}",
        contract_resp.contract_id, contract_resp.balance
    ));

    Ok(())
}

#[wasm_bindgen_test]
async fn accept_new_rgb21_transfer() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let issue_req = ContractSeed::uda(issuer_keys.clone(), none!());
    let issue_res = create_new_contract(issue_req).await?;
    let ContractResponse {
        contract_id,
        iface,
        contract,
        precision,
        ..
    } = issue_res[0].clone();

    let amount = ContractAmount::new(1, 0, precision);
    let rgb_amount = RgbInvoiceValue::UDA(UDAValue::with(1, 1));

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
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref owner_sk,
        ..
    } = owners_keys.private;

    let transfer_req = serde_wasm_bindgen::to_value(&transfer_req).expect("");

    let transfer_resp: JsValue =
        resolve(create_rgb_transfer(sender_sk.to_string(), transfer_req)).await;
    let transfer_resp: RgbTransferResponse = json_parse(&transfer_resp);

    let descriptors = vec![
        SecretString(btc_descriptor_xprv.clone()),
        SecretString(btc_change_descriptor_xprv.clone()),
        SecretString(rgb_descriptor_xprv.clone()),
    ];

    let psbt_req = SignPsbtRequest {
        psbt: transfer_resp.psbt,
        descriptors,
    };
    let psbt_req = serde_wasm_bindgen::to_value(&psbt_req).expect("");
    resolve(psbt_sign_and_publish_file(psbt_req)).await;

    let save_transfer_req = SaveTransferRequest {
        consignment: transfer_resp.consig.to_string(),
    };
    let save_transfer_req = serde_wasm_bindgen::to_value(&save_transfer_req).expect("");

    let save_transfer_resp: JsValue =
        resolve(save_transfer(owner_sk.to_string(), save_transfer_req)).await;
    let save_transfer_resp: RgbTransferItem = json_parse(&save_transfer_resp);

    let _ = new_block().await;
    resolve(verify_transfers(owner_sk.to_string())).await;

    let contract_resp = resolve(get_contract(
        owner_sk.to_string(),
        save_transfer_resp.contract_id,
    ))
    .await;

    let contract_resp: ContractResponse = json_parse(&contract_resp);

    debug!(format!("Contract Data"));
    debug!(format!(
        "{} @ {}",
        contract_resp.contract_id, contract_resp.balance
    ));

    Ok(())
}

#[wasm_bindgen_test]
async fn accept_new_rgb20_transfer_by_proxy() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB20;

    let amount = ContractAmount::new(1, 0, 2);
    let transfer_value = RgbInvoiceValue::Value(amount.to_string());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let seed = SeedTransfer::new(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
    );
    let RgbTransferResponse { consig, psbt, .. } = create_and_publish_transfer(seed).await?;

    info!("Create new Block");
    let _ = new_block().await;

    resolve(verify_transfers(beneficiary_sk.to_string())).await;

    let contract_resp = resolve(list_contracts(beneficiary_sk.to_string(), true)).await;
    let contract_resp: Vec<ContractResponse> = json_parse(&contract_resp);
    for contract_resp in contract_resp {
        debug!(format!("Contract Data"));
        debug!(format!(
            "{} @ {}",
            contract_resp.contract_id, contract_resp.balance
        ));

        assert_eq!(contract_resp.balance, ContractValue::Value(100));
    }
    Ok(())
}

#[wasm_bindgen_test]
async fn accept_new_rgb21_transfer_by_proxy() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB21;
    let transfer_value = RgbInvoiceValue::UDA(UDAValue::default());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let seed = SeedTransfer::new(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
    );
    let RgbTransferResponse { consig, psbt, .. } = create_and_publish_transfer(seed).await?;

    info!("Create new Block");
    let _ = new_block().await;

    resolve(verify_transfers(beneficiary_sk.to_string())).await;

    let contract_resp = resolve(list_contracts(beneficiary_sk.to_string(), true)).await;
    let contract_resp: Vec<ContractResponse> = json_parse(&contract_resp);
    for contract_resp in contract_resp {
        debug!(format!("Contract Data"));
        debug!(format!(
            "{} @ {}",
            contract_resp.contract_id, contract_resp.balance
        ));

        assert_eq!(
            contract_resp.balance,
            ContractValue::UDA(UDAValue::with(1, 1))
        );
    }
    Ok(())
}
