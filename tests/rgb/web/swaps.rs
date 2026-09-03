#![cfg(target_arch = "wasm32")]

use crate::rgb::web::seeder::{
    ContractSeed, WalletSeed, create_new_contract, create_new_fund_wallet,
};
use crate::rgb::web::utils::new_block;
use amplify::none;
use bitmask_core::debug;
use bitmask_core::rgb::models::{ContractAmount, ContractInfo, ContractKit};
use bitmask_core::rgb::swap::RgbSwap;
use bitmask_core::structs::{
    ChainFee, ContractCounterParty, ContractResponse, ContractValue, OrderValue, PrivateWalletData,
    RgbBidRequest, RgbBidResponse, RgbOfferItem, RgbOfferRequest, RgbOfferResponse, SecretString,
    UDAValue,
};
use bitmask_core::web::rgb::{
    bids, create_bid, create_offer, get_contract, list_offers, verify_transfers,
};
use bitmask_core::web::wallet::destroy_watcher;
use bitmask_core::web::{json_parse, resolve};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn create_swap_uda_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(WalletSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(WalletSeed::default()).await?;

    let PrivateWalletData {
        nostr_prv: ref sender_sk,
        btc_descriptor_xprv: ref sender_btc_prv,
        btc_change_descriptor_xprv: ref sender_btc_change_prv,
        rgb_descriptor_xprv: ref sender_rgb_prv,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref owner_sk,
        btc_descriptor_xprv: ref owner_btc_prv,
        btc_change_descriptor_xprv: ref owner_btc_change_prv,
        rgb_descriptor_xprv: ref owner_rgb_prv,
        ..
    } = owner_keys.private.clone();

    let mut issue_req = ContractSeed::uda(issuer_keys, none!());
    issue_req.options.proxy = false;

    let issue_res = create_new_contract(issue_req).await?;
    let ContractResponse { contract_id, .. } = issue_res[0].clone();

    let contract_amount = ContractAmount::new(1, 0, 0).to_string();
    let counter_party = OrderValue::bitcoin(1000);

    let req = RgbOfferRequest {
        strategy: RgbSwap::P2P,
        offers: [RgbOfferItem {
            contract_id: contract_id.clone(),
            contract_amount,
            counter_party,
            expire_at: None,
        }]
        .to_vec(),
    };

    let secrets = vec![
        SecretString(sender_btc_prv.to_string()),
        SecretString(sender_rgb_prv.to_string()),
        SecretString(sender_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid request");

    let resp: JsValue = resolve(create_offer(sender_sk.to_string(), req, secrets)).await;

    let new_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    let offer = new_offers[0].clone();

    let resp = resolve(list_offers(sender_sk.to_string())).await;
    let public_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    assert!(public_offers.iter().any(|x| x.id == offer.id));
    debug!(format!(
        "Offer swap: {}",
        public_offers.iter().any(|x| x.id == offer.id)
    ));

    let req = RgbBidRequest {
        offer_id: offer.id.to_string(),
        asset_price: OrderValue::bitcoin(1000),
        fee: ChainFee::Value(1000),
        bundle_id: None,
    };

    let secrets = vec![
        SecretString(owner_rgb_prv.to_string()),
        SecretString(owner_btc_prv.to_string()),
        SecretString(owner_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid secret");

    let resp: JsValue = resolve(create_bid(owner_sk.to_string(), req, secrets)).await;
    let new_bid: RgbBidResponse = json_parse(&resp);

    let resp = resolve(bids(owner_sk.to_string())).await;
    let my_bids: Vec<RgbBidResponse> = json_parse(&resp);

    assert!(my_bids.iter().any(|x| x.id == new_bid.id));
    debug!(format!(
        "Bid swap: {}",
        my_bids.iter().any(|x| x.id == new_bid.id)
    ));

    let _ = new_block().await;

    debug!(format!("Verify Transfers"));
    resolve(verify_transfers(owner_sk.to_string())).await;
    resolve(verify_transfers(sender_sk.to_string())).await;
    debug!(format!("Verify Transfers Complete"));

    let contract = resolve(get_contract(sender_sk.to_string(), contract_id.to_string())).await;
    let contract: ContractResponse = json_parse(&contract);
    debug!(format!("Sender swap: {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let contract = resolve(get_contract(owner_sk.to_string(), contract_id.to_string())).await;
    let contract: ContractResponse = json_parse(&contract);
    debug!(format!("Receiver swap: {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    resolve(destroy_watcher(sender_sk.to_string())).await;
    resolve(destroy_watcher(owner_sk.to_string())).await;

    Ok(())
}

#[wasm_bindgen_test]
async fn create_swap_fungible_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(WalletSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(WalletSeed::default()).await?;

    let PrivateWalletData {
        nostr_prv: ref sender_sk,
        btc_descriptor_xprv: ref sender_btc_prv,
        btc_change_descriptor_xprv: ref sender_btc_change_prv,
        rgb_descriptor_xprv: ref sender_rgb_prv,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref owner_sk,
        btc_descriptor_xprv: ref owner_btc_prv,
        btc_change_descriptor_xprv: ref owner_btc_change_prv,
        rgb_descriptor_xprv: ref owner_rgb_prv,
        ..
    } = owner_keys.private.clone();

    let issue_req = ContractSeed::fungible(issuer_keys, none!());
    let issue_res = create_new_contract(issue_req).await?;
    let ContractResponse { contract_id, .. } = issue_res[0].clone();

    let contract_amount = ContractAmount::new(1, 0, 2).to_string();
    let counter_party = OrderValue::bitcoin(1000);

    let req = RgbOfferRequest {
        strategy: RgbSwap::P2P,
        offers: [RgbOfferItem {
            contract_id: contract_id.clone(),
            contract_amount,
            counter_party,
            expire_at: None,
        }]
        .to_vec(),
    };

    let secrets = vec![
        SecretString(sender_btc_prv.to_string()),
        SecretString(sender_rgb_prv.to_string()),
        SecretString(sender_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid request");

    let resp: JsValue = resolve(create_offer(sender_sk.to_string(), req, secrets)).await;

    let new_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    let offer = new_offers[0].clone();

    let resp = resolve(list_offers(sender_sk.to_string())).await;
    let public_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    assert!(public_offers.iter().any(|x| x.id == offer.id));

    let req = RgbBidRequest {
        offer_id: offer.id.to_string(),
        asset_price: OrderValue::bitcoin(1000),
        fee: ChainFee::Value(1000),
        bundle_id: None,
    };

    let secrets = vec![
        SecretString(owner_rgb_prv.to_string()),
        SecretString(owner_btc_prv.to_string()),
        SecretString(owner_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid secret");

    let resp: JsValue = resolve(create_bid(owner_sk.to_string(), req, secrets)).await;
    let new_bid: RgbBidResponse = json_parse(&resp);

    let resp = resolve(bids(owner_sk.to_string())).await;
    let my_bids: Vec<RgbBidResponse> = json_parse(&resp);
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    let _ = new_block().await;

    resolve(verify_transfers(owner_sk.to_string())).await;
    resolve(verify_transfers(sender_sk.to_string())).await;

    let contract = resolve(get_contract(owner_sk.to_string(), contract_id.to_string())).await;
    let contract: ContractResponse = json_parse(&contract);
    debug!(format!("Receiver swap: {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(100));

    let contract = resolve(get_contract(sender_sk.to_string(), contract_id.to_string())).await;
    let contract: ContractResponse = json_parse(&contract);
    debug!(format!("Sender swap: {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(410));

    resolve(destroy_watcher(sender_sk.to_string())).await;
    resolve(destroy_watcher(owner_sk.to_string())).await;

    Ok(())
}

#[wasm_bindgen_test]
async fn create_swap_fungible_fungible() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(WalletSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(WalletSeed::default()).await?;

    let PrivateWalletData {
        nostr_prv: ref sender_sk,
        btc_descriptor_xprv: ref sender_btc_prv,
        btc_change_descriptor_xprv: ref sender_btc_change_prv,
        rgb_descriptor_xprv: ref sender_rgb_prv,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref owner_sk,
        btc_descriptor_xprv: ref owner_btc_prv,
        btc_change_descriptor_xprv: ref owner_btc_change_prv,
        rgb_descriptor_xprv: ref owner_rgb_prv,
        ..
    } = owner_keys.private.clone();

    let issue_req = ContractSeed::fungible(issuer_keys, none!());
    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_offer,
        contract: contract_offer,
        ..
    } = contract_resp.clone();

    let contract_info_offer = ContractInfo::with(contract_resp);
    let contract_kit_offer = ContractKit::new(contract_info_offer, contract_offer);

    let issue_req = ContractSeed::fungible(owner_keys, none!());
    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_bid,
        contract: contract_bid,
        ..
    } = contract_resp.clone();

    let contract_info_bid = ContractInfo::with(contract_resp);
    let contract_kit_bid = ContractKit::new(contract_info_bid, contract_bid);
    let contract_amount = ContractAmount::new(1, 0, 2);

    let counter_party = ContractCounterParty::with(contract_kit_bid, contract_amount);
    let counter_party = OrderValue::Contract(counter_party);

    let req = RgbOfferRequest {
        strategy: RgbSwap::P2P,
        offers: [RgbOfferItem {
            contract_id: contract_id_offer.clone(),
            contract_amount: contract_amount.to_string(),
            counter_party,
            expire_at: None,
        }]
        .to_vec(),
    };

    let secrets = vec![
        SecretString(sender_btc_prv.to_string()),
        SecretString(sender_rgb_prv.to_string()),
        SecretString(sender_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid request");

    let resp: JsValue = resolve(create_offer(sender_sk.to_string(), req, secrets)).await;

    let new_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    let offer = new_offers[0].clone();

    let resp = resolve(list_offers(sender_sk.to_string())).await;
    let public_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    assert!(public_offers.iter().any(|x| x.id == offer.id));

    let counter_party = ContractCounterParty::with(contract_kit_offer, contract_amount);
    let counter_party = OrderValue::Contract(counter_party);

    let req = RgbBidRequest {
        offer_id: offer.id.to_string(),
        asset_price: counter_party,
        fee: ChainFee::Value(500),
        bundle_id: None,
    };

    let secrets = vec![
        SecretString(owner_rgb_prv.to_string()),
        SecretString(owner_btc_prv.to_string()),
        SecretString(owner_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid secret");

    let resp: JsValue = resolve(create_bid(owner_sk.to_string(), req, secrets)).await;
    let new_bid: RgbBidResponse = json_parse(&resp);

    let resp = resolve(bids(owner_sk.to_string())).await;
    let my_bids: Vec<RgbBidResponse> = json_parse(&resp);

    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    let _ = new_block().await;

    resolve(verify_transfers(owner_sk.to_string())).await;
    resolve(verify_transfers(sender_sk.to_string())).await;

    let sender = resolve(get_contract(
        sender_sk.to_string(),
        contract_id_offer.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&sender);
    debug!(format!("Sender swap (offer): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(410));

    let receiver = resolve(get_contract(
        owner_sk.to_string(),
        contract_id_offer.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&receiver);
    debug!(format!("Receiver swap (offer): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(100));

    let sender = resolve(get_contract(
        sender_sk.to_string(),
        contract_id_bid.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&sender);
    debug!(format!("Sender swap (bid): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(100));

    let receiver = resolve(get_contract(
        owner_sk.to_string(),
        contract_id_bid.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&receiver);
    debug!(format!("Receiver swap (bid): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(410));
    resolve(destroy_watcher(sender_sk.to_string())).await;
    resolve(destroy_watcher(owner_sk.to_string())).await;
    Ok(())
}

#[wasm_bindgen_test]
async fn create_swap_uda_uda() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(WalletSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(WalletSeed::default()).await?;

    let PrivateWalletData {
        nostr_prv: ref sender_sk,
        btc_descriptor_xprv: ref sender_btc_prv,
        btc_change_descriptor_xprv: ref sender_btc_change_prv,
        rgb_descriptor_xprv: ref sender_rgb_prv,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref owner_sk,
        btc_descriptor_xprv: ref owner_btc_prv,
        btc_change_descriptor_xprv: ref owner_btc_change_prv,
        rgb_descriptor_xprv: ref owner_rgb_prv,
        ..
    } = owner_keys.private.clone();

    let mut issue_req = ContractSeed::uda(issuer_keys, none!());
    issue_req.options.proxy = false;

    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_offer,
        contract: contract_offer,
        ..
    } = contract_resp.clone();

    let contract_info_offer = ContractInfo::with(contract_resp);
    let contract_kit_offer = ContractKit::new(contract_info_offer, contract_offer);

    let mut issue_req = ContractSeed::uda(owner_keys, none!());
    issue_req.options.proxy = false;

    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_bid,
        contract: contract_bid,
        ..
    } = contract_resp.clone();

    let contract_info_bid = ContractInfo::with(contract_resp);
    let contract_kit_bid = ContractKit::new(contract_info_bid, contract_bid);
    let contract_amount = ContractAmount::new(1, 0, 0);

    let counter_party = ContractCounterParty::with(contract_kit_bid, contract_amount);
    let counter_party = OrderValue::Contract(counter_party);

    let req = RgbOfferRequest {
        strategy: RgbSwap::P2P,
        offers: [RgbOfferItem {
            contract_id: contract_id_offer.clone(),
            contract_amount: contract_amount.to_string(),
            counter_party,
            expire_at: None,
        }]
        .to_vec(),
    };

    let secrets = vec![
        SecretString(sender_btc_prv.to_string()),
        SecretString(sender_rgb_prv.to_string()),
        SecretString(sender_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid request");

    let resp: JsValue = resolve(create_offer(sender_sk.to_string(), req, secrets)).await;

    let new_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    let offer = new_offers[0].clone();

    let resp = resolve(list_offers(sender_sk.to_string())).await;
    let public_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    assert!(public_offers.iter().any(|x| x.id == offer.id));

    let counter_party = ContractCounterParty::with(contract_kit_offer, contract_amount);
    let counter_party = OrderValue::Contract(counter_party);

    let req = RgbBidRequest {
        offer_id: offer.id.to_string(),
        asset_price: counter_party,
        fee: ChainFee::Value(500),
        bundle_id: None,
    };

    let secrets = vec![
        SecretString(owner_rgb_prv.to_string()),
        SecretString(owner_btc_prv.to_string()),
        SecretString(owner_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid secret");

    let resp: JsValue = resolve(create_bid(owner_sk.to_string(), req, secrets)).await;
    let new_bid: RgbBidResponse = json_parse(&resp);

    let resp = resolve(bids(owner_sk.to_string())).await;
    let my_bids: Vec<RgbBidResponse> = json_parse(&resp);

    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    let _ = new_block().await;

    resolve(verify_transfers(owner_sk.to_string())).await;
    resolve(verify_transfers(sender_sk.to_string())).await;

    let sender = resolve(get_contract(
        sender_sk.to_string(),
        contract_id_offer.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&sender);
    debug!(format!("Sender swap (offer): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let receiver = resolve(get_contract(
        owner_sk.to_string(),
        contract_id_offer.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&receiver);
    debug!(format!("Receiver swap (offer): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let sender = resolve(get_contract(
        sender_sk.to_string(),
        contract_id_bid.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&sender);
    debug!(format!("Sender swap (bid): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let receiver = resolve(get_contract(
        owner_sk.to_string(),
        contract_id_bid.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&receiver);
    debug!(format!("Receiver swap (bid): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));
    resolve(destroy_watcher(sender_sk.to_string())).await;
    resolve(destroy_watcher(owner_sk.to_string())).await;
    Ok(())
}

#[wasm_bindgen_test]
async fn create_swap_fungible_uda() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(WalletSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(WalletSeed::default()).await?;

    let PrivateWalletData {
        nostr_prv: ref sender_sk,
        btc_descriptor_xprv: ref sender_btc_prv,
        btc_change_descriptor_xprv: ref sender_btc_change_prv,
        rgb_descriptor_xprv: ref sender_rgb_prv,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref owner_sk,
        btc_descriptor_xprv: ref owner_btc_prv,
        btc_change_descriptor_xprv: ref owner_btc_change_prv,
        rgb_descriptor_xprv: ref owner_rgb_prv,
        ..
    } = owner_keys.private.clone();

    let issue_req = ContractSeed::fungible(issuer_keys, none!());
    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_offer,
        contract: contract_offer,
        ..
    } = contract_resp.clone();

    let contract_info_offer = ContractInfo::with(contract_resp);
    let contract_kit_offer = ContractKit::new(contract_info_offer, contract_offer);

    let mut issue_req = ContractSeed::uda(owner_keys, none!());
    issue_req.options.proxy = false;

    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_bid,
        contract: contract_bid,
        ..
    } = contract_resp.clone();

    let contract_info_bid = ContractInfo::with(contract_resp);
    let contract_kit_bid = ContractKit::new(contract_info_bid, contract_bid);
    let contract_amount = ContractAmount::new(1, 0, 2);
    let cp_amount = ContractAmount::new(1, 0, 0);

    let counter_party = ContractCounterParty::with(contract_kit_bid, cp_amount);
    let counter_party = OrderValue::Contract(counter_party);

    let req = RgbOfferRequest {
        strategy: RgbSwap::P2P,
        offers: [RgbOfferItem {
            contract_id: contract_id_offer.clone(),
            contract_amount: contract_amount.to_string(),
            counter_party,
            expire_at: None,
        }]
        .to_vec(),
    };

    let secrets = vec![
        SecretString(sender_btc_prv.to_string()),
        SecretString(sender_rgb_prv.to_string()),
        SecretString(sender_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid request");

    let resp: JsValue = resolve(create_offer(sender_sk.to_string(), req, secrets)).await;

    let new_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    let offer = new_offers[0].clone();

    let resp = resolve(list_offers(sender_sk.to_string())).await;
    let public_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    assert!(public_offers.iter().any(|x| x.id == offer.id));

    let counter_party = ContractCounterParty::with(contract_kit_offer, contract_amount);
    let counter_party = OrderValue::Contract(counter_party);

    let req = RgbBidRequest {
        offer_id: offer.id.to_string(),
        asset_price: counter_party,
        fee: ChainFee::Value(500),
        bundle_id: None,
    };

    let secrets = vec![
        SecretString(owner_rgb_prv.to_string()),
        SecretString(owner_btc_prv.to_string()),
        SecretString(owner_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid secret");

    let resp: JsValue = resolve(create_bid(owner_sk.to_string(), req, secrets)).await;
    let new_bid: RgbBidResponse = json_parse(&resp);

    let resp = resolve(bids(owner_sk.to_string())).await;
    let my_bids: Vec<RgbBidResponse> = json_parse(&resp);

    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    let _ = new_block().await;

    resolve(verify_transfers(owner_sk.to_string())).await;
    resolve(verify_transfers(sender_sk.to_string())).await;

    let sender = resolve(get_contract(
        sender_sk.to_string(),
        contract_id_offer.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&sender);
    debug!(format!("Sender swap (offer): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(410));

    let receiver = resolve(get_contract(
        owner_sk.to_string(),
        contract_id_offer.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&receiver);
    debug!(format!("Receiver swap (offer): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(100));

    let sender = resolve(get_contract(
        sender_sk.to_string(),
        contract_id_bid.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&sender);
    debug!(format!("Sender swap (bid): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let receiver = resolve(get_contract(
        owner_sk.to_string(),
        contract_id_bid.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&receiver);
    debug!(format!("Receiver swap (bid): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    resolve(destroy_watcher(sender_sk.to_string())).await;
    resolve(destroy_watcher(owner_sk.to_string())).await;
    Ok(())
}

#[wasm_bindgen_test]
async fn create_swap_uda_fungible() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(WalletSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(WalletSeed::default()).await?;

    let PrivateWalletData {
        nostr_prv: ref sender_sk,
        btc_descriptor_xprv: ref sender_btc_prv,
        btc_change_descriptor_xprv: ref sender_btc_change_prv,
        rgb_descriptor_xprv: ref sender_rgb_prv,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref owner_sk,
        btc_descriptor_xprv: ref owner_btc_prv,
        btc_change_descriptor_xprv: ref owner_btc_change_prv,
        rgb_descriptor_xprv: ref owner_rgb_prv,
        ..
    } = owner_keys.private.clone();

    let mut issue_req = ContractSeed::uda(issuer_keys, none!());
    issue_req.options.proxy = false;

    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_offer,
        contract: contract_offer,
        ..
    } = contract_resp.clone();

    let contract_info_offer = ContractInfo::with(contract_resp);
    let contract_kit_offer = ContractKit::new(contract_info_offer, contract_offer);

    let issue_req = ContractSeed::fungible(owner_keys, none!());
    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_bid,
        contract: contract_bid,
        ..
    } = contract_resp.clone();

    let contract_info_bid = ContractInfo::with(contract_resp);
    let contract_kit_bid = ContractKit::new(contract_info_bid, contract_bid);
    let contract_amount = ContractAmount::new(1, 0, 0);
    let cp_amount = ContractAmount::new(1, 0, 2);

    let counter_party = ContractCounterParty::with(contract_kit_bid, cp_amount);
    let counter_party = OrderValue::Contract(counter_party);

    let req = RgbOfferRequest {
        strategy: RgbSwap::P2P,
        offers: [RgbOfferItem {
            contract_id: contract_id_offer.clone(),
            contract_amount: contract_amount.to_string(),
            counter_party,
            expire_at: None,
        }]
        .to_vec(),
    };

    let secrets = vec![
        SecretString(sender_btc_prv.to_string()),
        SecretString(sender_rgb_prv.to_string()),
        SecretString(sender_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid request");

    let resp: JsValue = resolve(create_offer(sender_sk.to_string(), req, secrets)).await;

    let new_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    let offer = new_offers[0].clone();

    let resp = resolve(list_offers(sender_sk.to_string())).await;
    let public_offers: Vec<RgbOfferResponse> = json_parse(&resp);
    assert!(public_offers.iter().any(|x| x.id == offer.id));

    let counter_party = ContractCounterParty::with(contract_kit_offer, contract_amount);
    let counter_party = OrderValue::Contract(counter_party);

    let req = RgbBidRequest {
        offer_id: offer.id.to_string(),
        asset_price: counter_party,
        fee: ChainFee::Value(500),
        bundle_id: None,
    };

    let secrets = vec![
        SecretString(owner_rgb_prv.to_string()),
        SecretString(owner_btc_prv.to_string()),
        SecretString(owner_btc_change_prv.to_string()),
    ];

    let req = serde_wasm_bindgen::to_value(&req).expect("invalid request");
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid secret");

    let resp: JsValue = resolve(create_bid(owner_sk.to_string(), req, secrets)).await;
    let new_bid: RgbBidResponse = json_parse(&resp);

    let resp = resolve(bids(owner_sk.to_string())).await;
    let my_bids: Vec<RgbBidResponse> = json_parse(&resp);

    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    let _ = new_block().await;

    resolve(verify_transfers(owner_sk.to_string())).await;
    resolve(verify_transfers(sender_sk.to_string())).await;

    let contract = resolve(get_contract(
        sender_sk.to_string(),
        contract_id_offer.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&contract);
    debug!(format!("Sender swap (offer): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let contract = resolve(get_contract(
        owner_sk.to_string(),
        contract_id_offer.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&contract);
    debug!(format!("Receiver swap (offer): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let contract = resolve(get_contract(
        sender_sk.to_string(),
        contract_id_bid.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&contract);
    debug!(format!("Sender swap (bid): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(100));

    let contract = resolve(get_contract(
        owner_sk.to_string(),
        contract_id_bid.to_string(),
    ))
    .await;
    let contract: ContractResponse = json_parse(&contract);
    debug!(format!("Receiver swap (bid): {:#?}", contract.balance));
    assert_eq!(contract.balance, ContractValue::Value(410));

    resolve(destroy_watcher(sender_sk.to_string())).await;
    resolve(destroy_watcher(owner_sk.to_string())).await;

    Ok(())
}
