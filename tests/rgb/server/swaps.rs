#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::server::seeder::{ContractSeed, create_new_contract, create_new_fund_wallet};
use crate::rgb::server::seeder::{FundSeed, get_uri_media};
use std::str::FromStr;

use amplify::none;
use bitmask_core::regtest::new_block;
use bitmask_core::rgb::models::{ContractAmount, ContractInfo, ContractKit};
use bitmask_core::rgb::swap::{BidOptions, OfferOptions, RgbSwap};
use bitmask_core::rgb::{
    bids, create_bid, create_offer, get_contract, public_offers, verify_transfers,
};
use bitmask_core::structs::{
    ChainFee, ContractCounterParty, ContractResponse, ContractValue, OrderValue, PrivateWalletData,
    PublicWalletData, RgbBidRequest, RgbOfferItem, RgbOfferRequest, SecretString, UDAValue,
};
use rgb::ContractId;

#[derive(Clone, PartialEq)]
pub enum Direction {
    Issuer,
    Owner,
    Other,
}

#[allow(clippy::large_enum_variant)] // test helper holds ContractResponse
#[derive(PartialEq)]
pub enum Pay {
    Bitcoin(u64),
    #[allow(dead_code)]
    Contract {
        amount: ContractAmount,
        contract: ContractResponse,
    },
}

#[derive(PartialEq)]
pub struct PayRound {
    dir: Direction,
    delivery: ContractAmount,
    payment: Pay,
}

#[derive(PartialEq)]
pub struct MultiPayRound {
    src: Direction,
    dst: Direction,
    delivery: ContractAmount,
    payment: Pay,
}

#[tokio::test]
async fn create_swap_uda_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

    let PrivateWalletData {
        nostr_prv: ref sender_sk,
        rgb_descriptor_xprv: ref sender_rgb_prv,
        ..
    } = issuer_keys.private.clone();
    let PublicWalletData { .. } = issuer_keys.public.clone();

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

    let secrets = vec![SecretString(sender_rgb_prv.to_string())];

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let contract = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let contract = get_contract(&owner_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));
    Ok(())
}

#[tokio::test]
async fn create_swap_fungible_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

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

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let contract = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(410));

    let contract = get_contract(&owner_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(100));
    Ok(())
}

#[tokio::test]
async fn create_swap_fungible_fungible() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

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

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let contract = get_contract(&sender_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(410));

    let contract = get_contract(&owner_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(100));

    let contract = get_contract(&sender_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(100));

    let contract = get_contract(&owner_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(410));
    Ok(())
}

#[tokio::test]
async fn create_swap_uda_uda() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

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

    let issue_req = ContractSeed::uda(issuer_keys, Some(get_uri_media()));
    let issue_res = create_new_contract(issue_req).await?;
    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_offer,
        contract: contract_offer,
        ..
    } = contract_resp.clone();

    let contract_info_offer = ContractInfo::with(contract_resp);
    let contract_kit_offer = ContractKit::new(contract_info_offer, contract_offer);

    let issue_req = ContractSeed::uda(owner_keys, Some(get_uri_media()));
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

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let contract = get_contract(&sender_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let contract = get_contract(&owner_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let contract = get_contract(&sender_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let contract = get_contract(&owner_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));
    Ok(())
}

#[tokio::test]
async fn create_swap_fungible_uda() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

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

    let issue_req = ContractSeed::uda(owner_keys, Some(get_uri_media()));
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

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let contract = get_contract(&sender_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(410));

    let contract = get_contract(&owner_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(100));

    let contract = get_contract(&sender_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let contract = get_contract(&owner_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));
    Ok(())
}

#[tokio::test]
async fn create_swap_uda_fungible() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

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

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let offer_sender = get_contract(&sender_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(
        offer_sender.balance,
        ContractValue::UDA(UDAValue::with(1, 0))
    );

    let offer_receiver = get_contract(&owner_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(
        offer_receiver.balance,
        ContractValue::UDA(UDAValue::with(1, 1))
    );

    let bid_sender = get_contract(&sender_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(bid_sender.balance, ContractValue::Value(100));

    let bid_receiver = get_contract(&owner_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(bid_receiver.balance, ContractValue::Value(410));
    Ok(())
}

#[tokio::test]
async fn create_swap_extra_fungible_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

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

    let mut issue_req = ContractSeed::fungible(issuer_keys, none!());
    issue_req.quantity = 2;

    let issue_res = create_new_contract(issue_req).await?;
    let ContractResponse { contract_id, .. } = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_2,
        ..
    } = issue_res[1].clone();

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

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let contract = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(410));

    let contract = get_contract(&owner_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(100));

    let contract = get_contract(&sender_sk.to_string(), contract_id_2.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(510));
    Ok(())
}

#[tokio::test]
async fn create_swap_extra_uda_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

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
    issue_req.quantity = 2;

    let issue_res = create_new_contract(issue_req).await?;
    let ContractResponse { contract_id, .. } = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_2,
        ..
    } = issue_res[1].clone();

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

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let contract = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let contract = get_contract(&owner_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let contract = get_contract(&sender_sk.to_string(), contract_id_2.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));
    Ok(())
}

#[tokio::test]
async fn create_swap_extra_fungible_fungible() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

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

    let mut issue_req = ContractSeed::fungible(issuer_keys, none!());
    issue_req.quantity = 2;

    let issue_res = create_new_contract(issue_req).await?;
    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_offer,
        contract: contract_offer,
        ..
    } = contract_resp.clone();

    let ContractResponse {
        contract_id: contract_id_offer_2,
        ..
    } = issue_res[1].clone();

    let contract_info_offer = ContractInfo::with(contract_resp);
    let contract_kit_offer = ContractKit::new(contract_info_offer, contract_offer);

    let mut issue_req = ContractSeed::fungible(owner_keys, none!());
    issue_req.quantity = 2;

    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_bid,
        contract: contract_bid,
        ..
    } = contract_resp.clone();

    let ContractResponse {
        contract_id: contract_id_bid_2,
        ..
    } = issue_res[1].clone();

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

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let contract = get_contract(&sender_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(410));

    let contract = get_contract(&owner_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(100));

    let contract = get_contract(&sender_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(100));

    let contract = get_contract(&owner_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(410));

    let contract = get_contract(&sender_sk.to_string(), contract_id_offer_2.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(510));

    let contract = get_contract(&owner_sk.to_string(), contract_id_bid_2.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::Value(510));
    Ok(())
}

#[tokio::test]
async fn create_swap_extra_uda_uda() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

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

    let mut issue_req = ContractSeed::uda(issuer_keys, Some(get_uri_media()));
    issue_req.quantity = 2;

    let issue_res = create_new_contract(issue_req).await?;
    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_offer,
        contract: contract_offer,
        ..
    } = contract_resp.clone();

    let ContractResponse {
        contract_id: contract_id_offer_2,
        ..
    } = issue_res[1].clone();

    let contract_info_offer = ContractInfo::with(contract_resp);
    let contract_kit_offer = ContractKit::new(contract_info_offer, contract_offer);

    let mut issue_req = ContractSeed::uda(owner_keys, Some(get_uri_media()));
    issue_req.quantity = 2;

    let issue_res = create_new_contract(issue_req).await?;

    let contract_resp = issue_res[0].clone();
    let ContractResponse {
        contract_id: contract_id_bid,
        contract: contract_bid,
        ..
    } = contract_resp.clone();

    let ContractResponse {
        contract_id: contract_id_bid_2,
        ..
    } = issue_res[1].clone();

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

    let options = OfferOptions::p2p(secrets);
    let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

    let offer = new_offers[0].clone();

    let public_offers = public_offers(&sender_sk.to_string()).await?;
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

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    new_block();

    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&sender_sk.to_string(), true).await?;

    let contract = get_contract(&sender_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let contract = get_contract(&owner_sk.to_string(), contract_id_offer.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let contract = get_contract(&sender_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let contract = get_contract(&owner_sk.to_string(), contract_id_bid.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let contract = get_contract(&sender_sk.to_string(), contract_id_offer_2.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));

    let contract = get_contract(&owner_sk.to_string(), contract_id_bid_2.to_string()).await?;
    assert_eq!(contract.balance, ContractValue::UDA(UDAValue::with(1, 1)));
    Ok(())
}

#[tokio::test]
async fn create_consecutive_swap_fungible_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

    let issue_req = ContractSeed::fungible(issuer_keys.clone(), none!());
    let issue_res = create_new_contract(issue_req).await?;

    let offer_contract: ContractResponse = issue_res[0].clone();
    let ContractResponse { contract_id, .. } = offer_contract.clone();

    let contract_id = ContractId::from_str(&contract_id).expect("invalid contract");
    let contract_amount = ContractAmount::new(1, 0, 2);

    let rounds = [
        PayRound {
            dir: Direction::Owner,
            delivery: contract_amount,
            payment: Pay::Bitcoin(900),
        },
        PayRound {
            dir: Direction::Owner,
            delivery: contract_amount,
            payment: Pay::Bitcoin(1000),
        },
        PayRound {
            dir: Direction::Issuer,
            delivery: contract_amount,
            payment: Pay::Bitcoin(400),
        },
    ];

    let mut total_d_fungible_issuer = offer_contract.balance;
    let mut total_d_fungible_owner = ContractValue::Value(0);

    for round in rounds {
        let PayRound {
            dir,
            delivery: contract_amount,
            payment,
        } = round;

        let (sender_keys, receiver_keys) = if dir == Direction::Owner {
            (issuer_keys.clone(), owner_keys.clone())
        } else {
            (owner_keys.clone(), issuer_keys.clone())
        };

        let PrivateWalletData {
            nostr_prv: ref sender_sk,
            btc_descriptor_xprv: ref sender_btc_prv,
            btc_change_descriptor_xprv: ref sender_btc_change_prv,
            rgb_descriptor_xprv: ref sender_rgb_prv,
            ..
        } = sender_keys.private.clone();

        let PrivateWalletData {
            nostr_prv: ref receiver_sk,
            btc_descriptor_xprv: ref receiver_btc_prv,
            btc_change_descriptor_xprv: ref receiver_btc_change_prv,
            rgb_descriptor_xprv: ref receiver_rgb_prv,
            ..
        } = receiver_keys.private.clone();

        let counter_party = match payment {
            Pay::Bitcoin(sats) => OrderValue::Bitcoin(sats),
            Pay::Contract { contract, amount } => {
                let info = ContractInfo::with(contract.clone());
                let kit = ContractKit::new(info, contract.contract);
                let cp = ContractCounterParty::with(kit, amount);
                OrderValue::Contract(cp)
            }
        };

        let req = RgbOfferRequest {
            strategy: RgbSwap::P2P,
            offers: [RgbOfferItem {
                contract_id: contract_id.to_string(),
                contract_amount: contract_amount.to_string(),
                counter_party: counter_party.clone(),
                expire_at: None,
            }]
            .to_vec(),
        };

        let secrets = vec![
            SecretString(sender_btc_prv.to_string()),
            SecretString(sender_rgb_prv.to_string()),
            SecretString(sender_btc_change_prv.to_string()),
        ];

        let options = OfferOptions::p2p(secrets);
        let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

        let offer = new_offers[0].clone();

        let public_offers = public_offers(&sender_sk.to_string()).await?;
        assert!(public_offers.iter().any(|x| x.id == offer.id));

        let req = RgbBidRequest {
            offer_id: offer.id.to_string(),
            asset_price: counter_party.clone(),
            fee: ChainFee::Value(300),
            bundle_id: None,
        };

        let secrets = vec![
            SecretString(receiver_rgb_prv.to_string()),
            SecretString(receiver_btc_prv.to_string()),
            SecretString(receiver_btc_change_prv.to_string()),
        ];

        let options = BidOptions::p2p(secrets);
        let new_bid = create_bid(&receiver_sk.to_string(), req, options).await?;

        let my_bids = bids(&receiver_sk.to_string()).await?;
        assert!(my_bids.iter().any(|x| x.id == new_bid.id));

        new_block();

        verify_transfers(&receiver_sk.to_string(), true).await?;
        verify_transfers(&sender_sk.to_string(), true).await?;

        if dir == Direction::Owner {
            total_d_fungible_issuer = match total_d_fungible_issuer {
                ContractValue::Value(value) => {
                    ContractValue::Value(value - contract_amount.to_value())
                }
                _ => ContractValue::UDA(UDAValue::with(1, 0)),
            };

            total_d_fungible_owner = match total_d_fungible_owner {
                ContractValue::Value(value) => {
                    ContractValue::Value(value + contract_amount.to_value())
                }
                _ => ContractValue::UDA(UDAValue::with(1, 1)),
            };

            let contract = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
            // println!(
            //     "Issuer (sender): {:#?}",
            //     contract
            //         .allocations
            //         .into_iter()
            //         .map(|x| format!("{}@{}", x.utxo, x.value))
            // );

            assert_eq!(contract.balance, total_d_fungible_issuer);

            let contract = get_contract(&receiver_sk.to_string(), contract_id.to_string()).await?;
            // println!(
            //     "Owner (receiver): {:#?}",
            //     contract
            //         .allocations
            //         .into_iter()
            //         .map(|x| format!("{}@{}", x.utxo, x.value))
            // );
            assert_eq!(contract.balance, total_d_fungible_owner);
        } else {
            total_d_fungible_owner = match total_d_fungible_owner {
                ContractValue::Value(value) => {
                    ContractValue::Value(value - contract_amount.to_value())
                }
                _ => ContractValue::UDA(UDAValue::with(1, 0)),
            };

            total_d_fungible_issuer = match total_d_fungible_issuer {
                ContractValue::Value(value) => {
                    ContractValue::Value(value + contract_amount.to_value())
                }
                _ => ContractValue::UDA(UDAValue::with(1, 1)),
            };

            let contract = get_contract(&receiver_sk.to_string(), contract_id.to_string()).await?;
            // println!(
            //     "Issuer (receiver): {:#?}",
            //     contract
            //         .allocations
            //         .into_iter()
            //         .map(|x| format!("{}@{}", x.utxo, x.value))
            // );
            assert_eq!(contract.balance, total_d_fungible_issuer);

            let contract: ContractResponse =
                get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
            // println!(
            //     "Owner (sender): {:#?}",
            //     contract
            //         .allocations
            //         .into_iter()
            //         .map(|x| format!("{}@{}", x.utxo, x.value))
            // );
            assert_eq!(contract.balance, total_d_fungible_owner);
        };
    }

    Ok(())
}

#[tokio::test]
async fn create_consecutive_swap_uda_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

    let issue_req = ContractSeed::uda(issuer_keys.clone(), Some(get_uri_media()));
    let issue_res = create_new_contract(issue_req).await?;

    let offer_contract: ContractResponse = issue_res[0].clone();
    let ContractResponse { contract_id, .. } = offer_contract.clone();

    let contract_id = ContractId::from_str(&contract_id).expect("invalid contract");
    let contract_amount = ContractAmount::new(1, 0, 0);

    let rounds = [
        PayRound {
            dir: Direction::Owner,
            delivery: contract_amount,
            payment: Pay::Bitcoin(900),
        },
        PayRound {
            dir: Direction::Issuer,
            delivery: contract_amount,
            payment: Pay::Bitcoin(800),
        },
        PayRound {
            dir: Direction::Owner,
            delivery: contract_amount,
            payment: Pay::Bitcoin(800),
        },
    ];

    let mut total_d_fungible_issuer = offer_contract.balance;
    let mut total_d_fungible_owner = ContractValue::UDA(UDAValue::with(1, 0));

    for round in rounds {
        let PayRound {
            dir,
            delivery: contract_amount,
            payment,
        } = round;

        let (sender_keys, receiver_keys) = if dir == Direction::Owner {
            (issuer_keys.clone(), owner_keys.clone())
        } else {
            (owner_keys.clone(), issuer_keys.clone())
        };

        let PrivateWalletData {
            nostr_prv: ref sender_sk,
            btc_descriptor_xprv: ref sender_btc_prv,
            btc_change_descriptor_xprv: ref sender_btc_change_prv,
            rgb_descriptor_xprv: ref sender_rgb_prv,
            ..
        } = sender_keys.private.clone();

        let PrivateWalletData {
            nostr_prv: ref receiver_sk,
            btc_descriptor_xprv: ref receiver_btc_prv,
            btc_change_descriptor_xprv: ref receiver_btc_change_prv,
            rgb_descriptor_xprv: ref receiver_rgb_prv,
            ..
        } = receiver_keys.private.clone();

        let counter_party = match payment {
            Pay::Bitcoin(sats) => OrderValue::Bitcoin(sats),
            Pay::Contract { contract, amount } => {
                let info = ContractInfo::with(contract.clone());
                let kit = ContractKit::new(info, contract.contract);
                let cp = ContractCounterParty::with(kit, amount);
                OrderValue::Contract(cp)
            }
        };

        let req = RgbOfferRequest {
            strategy: RgbSwap::P2P,
            offers: [RgbOfferItem {
                contract_id: contract_id.to_string(),
                contract_amount: contract_amount.to_string(),
                counter_party: counter_party.clone(),
                expire_at: None,
            }]
            .to_vec(),
        };

        let secrets = vec![
            SecretString(sender_btc_prv.to_string()),
            SecretString(sender_rgb_prv.to_string()),
            SecretString(sender_btc_change_prv.to_string()),
        ];

        let options = OfferOptions::p2p(secrets);
        let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

        let offer = new_offers[0].clone();

        let public_offers = public_offers(&sender_sk.to_string()).await?;
        assert!(public_offers.iter().any(|x| x.id == offer.id));

        let req = RgbBidRequest {
            offer_id: offer.id.to_string(),
            asset_price: counter_party.clone(),
            fee: ChainFee::Value(350),
            bundle_id: None,
        };

        let secrets = vec![
            SecretString(receiver_rgb_prv.to_string()),
            SecretString(receiver_btc_prv.to_string()),
            SecretString(receiver_btc_change_prv.to_string()),
        ];

        let options = BidOptions::p2p(secrets);
        let new_bid = create_bid(&receiver_sk.to_string(), req, options).await?;

        let my_bids = bids(&receiver_sk.to_string()).await?;
        assert!(my_bids.iter().any(|x| x.id == new_bid.id));

        new_block();

        verify_transfers(&receiver_sk.to_string(), true).await?;
        verify_transfers(&sender_sk.to_string(), true).await?;

        if dir == Direction::Owner {
            total_d_fungible_issuer = match total_d_fungible_issuer {
                ContractValue::Value(value) => {
                    ContractValue::Value(value - contract_amount.to_value())
                }
                _ => ContractValue::UDA(UDAValue::with(1, 0)),
            };

            total_d_fungible_owner = match total_d_fungible_owner {
                ContractValue::Value(value) => {
                    ContractValue::Value(value + contract_amount.to_value())
                }
                _ => ContractValue::UDA(UDAValue::with(1, 1)),
            };

            let contract = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
            // println!(
            //     "Issuer (sender): {:#?}",
            //     contract
            //         .allocations
            //         .into_iter()
            //         .map(|x| format!("{}@{}", x.utxo, x.value))
            // );

            assert_eq!(contract.balance, total_d_fungible_issuer);

            let contract = get_contract(&receiver_sk.to_string(), contract_id.to_string()).await?;
            // println!(
            //     "Owner (receiver): {:#?}",
            //     contract
            //         .allocations
            //         .into_iter()
            //         .map(|x| format!("{}@{}", x.utxo, x.value))
            // );
            assert_eq!(contract.balance, total_d_fungible_owner);
        } else {
            total_d_fungible_owner = match total_d_fungible_owner {
                ContractValue::Value(value) => {
                    ContractValue::Value(value - contract_amount.to_value())
                }
                _ => ContractValue::UDA(UDAValue::with(1, 0)),
            };

            total_d_fungible_issuer = match total_d_fungible_issuer {
                ContractValue::Value(value) => {
                    ContractValue::Value(value + contract_amount.to_value())
                }
                _ => ContractValue::UDA(UDAValue::with(1, 1)),
            };

            let contract = get_contract(&receiver_sk.to_string(), contract_id.to_string()).await?;
            // println!(
            //     "Issuer (receiver): {:#?}",
            //     contract
            //         .allocations
            //         .into_iter()
            //         .map(|x| format!("{}@{}", x.utxo, x.value))
            // );
            assert_eq!(contract.balance, total_d_fungible_issuer);

            let contract: ContractResponse =
                get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
            // println!(
            //     "Owner (sender): {:#?}",
            //     contract
            //         .allocations
            //         .into_iter()
            //         .map(|x| format!("{}@{}", x.utxo, x.value))
            // );
            assert_eq!(contract.balance, total_d_fungible_owner);
        };
    }

    Ok(())
}

#[tokio::test]
async fn create_multiparty_consecutive_swap_uda_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let other_keys = create_new_fund_wallet(FundSeed::default()).await?;

    let issue_req = ContractSeed::uda(issuer_keys.clone(), Some(get_uri_media()));
    let issue_res = create_new_contract(issue_req).await?;

    let offer_contract: ContractResponse = issue_res[0].clone();
    let ContractResponse { contract_id, .. } = offer_contract.clone();

    let contract_id = ContractId::from_str(&contract_id).expect("invalid contract");
    let contract_amount = ContractAmount::new(1, 0, 0);

    let rounds = [
        MultiPayRound {
            src: Direction::Issuer,
            dst: Direction::Owner,
            delivery: contract_amount,
            payment: Pay::Bitcoin(900),
        },
        MultiPayRound {
            src: Direction::Owner,
            dst: Direction::Other,
            delivery: contract_amount,
            payment: Pay::Bitcoin(800),
        },
        MultiPayRound {
            src: Direction::Other,
            dst: Direction::Owner,
            delivery: contract_amount,
            payment: Pay::Bitcoin(800),
        },
        MultiPayRound {
            src: Direction::Owner,
            dst: Direction::Issuer,
            delivery: contract_amount,
            payment: Pay::Bitcoin(800),
        },
    ];

    let mut total_d_issuer = offer_contract.balance;
    let mut total_d_owner = ContractValue::UDA(UDAValue::with(1, 0));
    let mut total_d_other = ContractValue::UDA(UDAValue::with(1, 0));

    for round in rounds {
        let MultiPayRound {
            src,
            dst,
            delivery: contract_amount,
            payment,
        } = round;

        let (sender_keys, receiver_keys) = match (src.clone(), dst.clone()) {
            (Direction::Issuer, Direction::Owner) => (issuer_keys.clone(), owner_keys.clone()),
            (Direction::Issuer, Direction::Other) => (issuer_keys.clone(), other_keys.clone()),
            (Direction::Owner, Direction::Issuer) => (owner_keys.clone(), issuer_keys.clone()),
            (Direction::Owner, Direction::Other) => (owner_keys.clone(), other_keys.clone()),
            (Direction::Other, Direction::Issuer) => (other_keys.clone(), issuer_keys.clone()),
            (Direction::Other, Direction::Owner) => (other_keys.clone(), owner_keys.clone()),
            _ => panic!("invalid combination"),
        };

        let PrivateWalletData {
            nostr_prv: ref sender_sk,
            btc_descriptor_xprv: ref sender_btc_prv,
            btc_change_descriptor_xprv: ref sender_btc_change_prv,
            rgb_descriptor_xprv: ref sender_rgb_prv,
            ..
        } = sender_keys.private.clone();

        let PrivateWalletData {
            nostr_prv: ref receiver_sk,
            btc_descriptor_xprv: ref receiver_btc_prv,
            btc_change_descriptor_xprv: ref receiver_btc_change_prv,
            rgb_descriptor_xprv: ref receiver_rgb_prv,
            ..
        } = receiver_keys.private.clone();

        let counter_party = match payment {
            Pay::Bitcoin(sats) => OrderValue::Bitcoin(sats),
            Pay::Contract { contract, amount } => {
                let info = ContractInfo::with(contract.clone());
                let kit = ContractKit::new(info, contract.contract);
                let cp = ContractCounterParty::with(kit, amount);
                OrderValue::Contract(cp)
            }
        };

        let req = RgbOfferRequest {
            strategy: RgbSwap::P2P,
            offers: [RgbOfferItem {
                contract_id: contract_id.to_string(),
                contract_amount: contract_amount.to_string(),
                counter_party: counter_party.clone(),
                expire_at: None,
            }]
            .to_vec(),
        };

        let secrets = vec![
            SecretString(sender_btc_prv.to_string()),
            SecretString(sender_rgb_prv.to_string()),
            SecretString(sender_btc_change_prv.to_string()),
        ];

        let options = OfferOptions::p2p(secrets);
        let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

        let offer = new_offers[0].clone();

        let public_offers = public_offers(&sender_sk.to_string()).await?;
        assert!(public_offers.iter().any(|x| x.id == offer.id));

        let req = RgbBidRequest {
            offer_id: offer.id.to_string(),
            asset_price: counter_party.clone(),
            fee: ChainFee::Value(350),
            bundle_id: None,
        };

        let secrets = vec![
            SecretString(receiver_rgb_prv.to_string()),
            SecretString(receiver_btc_prv.to_string()),
            SecretString(receiver_btc_change_prv.to_string()),
        ];

        let options = BidOptions::p2p(secrets);
        let new_bid = create_bid(&receiver_sk.to_string(), req, options).await?;

        let my_bids = bids(&receiver_sk.to_string()).await?;
        assert!(my_bids.iter().any(|x| x.id == new_bid.id));

        new_block();

        verify_transfers(&receiver_sk.to_string(), true).await?;
        verify_transfers(&sender_sk.to_string(), true).await?;

        let total_sender = match src {
            Direction::Issuer => {
                total_d_issuer = match &total_d_issuer {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value - contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 0)),
                };
                total_d_issuer.clone()
            }
            Direction::Owner => {
                total_d_owner = match &total_d_owner {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value - contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 0)),
                };
                total_d_owner.clone()
            }
            Direction::Other => {
                total_d_other = match &total_d_other {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value - contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 0)),
                };
                total_d_other.clone()
            }
        };

        let contract = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
        assert_eq!(contract.balance, total_sender);
        // println!(
        //     "Sender: {:#?}",
        //     contract
        //         .allocations
        //         .into_iter()
        //         .map(|x| format!("{}@{}", x.utxo, x.value))
        // );

        let total_receiver = match dst {
            Direction::Issuer => {
                total_d_issuer = match &total_d_issuer {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value + contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 1)),
                };
                total_d_issuer.clone()
            }
            Direction::Owner => {
                total_d_owner = match &total_d_owner {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value + contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 1)),
                };
                total_d_owner.clone()
            }
            Direction::Other => {
                total_d_other = match &total_d_other {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value + contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 1)),
                };
                total_d_other.clone()
            }
        };

        let contract = get_contract(&receiver_sk.to_string(), contract_id.to_string()).await?;
        assert_eq!(contract.balance, total_receiver);
        // println!(
        //     "Receiver: {:#?}",
        //     contract
        //         .allocations
        //         .into_iter()
        //         .map(|x| format!("{}@{}", x.utxo, x.value))
        // );
    }

    Ok(())
}

#[tokio::test]
async fn create_multiparty_consecutive_swap_fungible_bitcoin() -> anyhow::Result<()> {
    let issuer_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;
    let other_keys = create_new_fund_wallet(FundSeed::default()).await?;

    let issue_req = ContractSeed::fungible(issuer_keys.clone(), none!());
    let issue_res = create_new_contract(issue_req).await?;

    let offer_contract: ContractResponse = issue_res[0].clone();
    let ContractResponse { contract_id, .. } = offer_contract.clone();

    let contract_id = ContractId::from_str(&contract_id).expect("invalid contract");
    let contract_amount = ContractAmount::new(1, 0, 2);

    let rounds = [
        MultiPayRound {
            src: Direction::Issuer,
            dst: Direction::Owner,
            delivery: contract_amount,
            payment: Pay::Bitcoin(900),
        },
        MultiPayRound {
            src: Direction::Issuer,
            dst: Direction::Other,
            delivery: contract_amount,
            payment: Pay::Bitcoin(600),
        },
        MultiPayRound {
            src: Direction::Other,
            dst: Direction::Issuer,
            delivery: contract_amount,
            payment: Pay::Bitcoin(600),
        },
        MultiPayRound {
            src: Direction::Issuer,
            dst: Direction::Owner,
            delivery: contract_amount,
            payment: Pay::Bitcoin(600),
        },
    ];

    let mut total_d_issuer = offer_contract.balance;
    let mut total_d_owner = ContractValue::Value(0);
    let mut total_d_other = ContractValue::Value(0);

    for round in rounds {
        let MultiPayRound {
            src,
            dst,
            delivery: contract_amount,
            payment,
        } = round;

        let (sender_keys, receiver_keys) = match (src.clone(), dst.clone()) {
            (Direction::Issuer, Direction::Owner) => (issuer_keys.clone(), owner_keys.clone()),
            (Direction::Issuer, Direction::Other) => (issuer_keys.clone(), other_keys.clone()),
            (Direction::Owner, Direction::Issuer) => (owner_keys.clone(), issuer_keys.clone()),
            (Direction::Owner, Direction::Other) => (owner_keys.clone(), other_keys.clone()),
            (Direction::Other, Direction::Issuer) => (other_keys.clone(), issuer_keys.clone()),
            (Direction::Other, Direction::Owner) => (other_keys.clone(), owner_keys.clone()),
            _ => panic!("invalid combination"),
        };

        let PrivateWalletData {
            nostr_prv: ref sender_sk,
            btc_descriptor_xprv: ref sender_btc_prv,
            btc_change_descriptor_xprv: ref sender_btc_change_prv,
            rgb_descriptor_xprv: ref sender_rgb_prv,
            ..
        } = sender_keys.private.clone();

        let PrivateWalletData {
            nostr_prv: ref receiver_sk,
            btc_descriptor_xprv: ref receiver_btc_prv,
            btc_change_descriptor_xprv: ref receiver_btc_change_prv,
            rgb_descriptor_xprv: ref receiver_rgb_prv,
            ..
        } = receiver_keys.private.clone();

        let counter_party = match payment {
            Pay::Bitcoin(sats) => OrderValue::Bitcoin(sats),
            Pay::Contract { contract, amount } => {
                let info = ContractInfo::with(contract.clone());
                let kit = ContractKit::new(info, contract.contract);
                let cp = ContractCounterParty::with(kit, amount);
                OrderValue::Contract(cp)
            }
        };

        let req = RgbOfferRequest {
            strategy: RgbSwap::P2P,
            offers: [RgbOfferItem {
                contract_id: contract_id.to_string(),
                contract_amount: contract_amount.to_string(),
                counter_party: counter_party.clone(),
                expire_at: None,
            }]
            .to_vec(),
        };

        let secrets = vec![
            SecretString(sender_btc_prv.to_string()),
            SecretString(sender_rgb_prv.to_string()),
            SecretString(sender_btc_change_prv.to_string()),
        ];

        let options = OfferOptions::p2p(secrets);
        let new_offers = create_offer(&sender_sk.to_string(), req, options).await?;

        let offer = new_offers[0].clone();

        let public_offers = public_offers(&sender_sk.to_string()).await?;
        assert!(public_offers.iter().any(|x| x.id == offer.id));

        let req = RgbBidRequest {
            offer_id: offer.id.to_string(),
            asset_price: counter_party.clone(),
            fee: ChainFee::Value(350),
            bundle_id: None,
        };

        let secrets = vec![
            SecretString(receiver_rgb_prv.to_string()),
            SecretString(receiver_btc_prv.to_string()),
            SecretString(receiver_btc_change_prv.to_string()),
        ];

        let options = BidOptions::p2p(secrets);
        let new_bid = create_bid(&receiver_sk.to_string(), req, options).await?;

        let my_bids = bids(&receiver_sk.to_string()).await?;
        assert!(my_bids.iter().any(|x| x.id == new_bid.id));

        new_block();

        verify_transfers(&receiver_sk.to_string(), true).await?;
        verify_transfers(&sender_sk.to_string(), true).await?;

        let total_sender = match src {
            Direction::Issuer => {
                total_d_issuer = match &total_d_issuer {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value - contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 0)),
                };
                total_d_issuer.clone()
            }
            Direction::Owner => {
                total_d_owner = match &total_d_owner {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value - contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 0)),
                };
                total_d_owner.clone()
            }
            Direction::Other => {
                total_d_other = match &total_d_other {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value - contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 0)),
                };
                total_d_other.clone()
            }
        };

        let contract = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
        assert_eq!(contract.balance, total_sender);
        // println!(
        //     "Sender: {:#?}",
        //     contract
        //         .allocations
        //         .into_iter()
        //         .map(|x| format!("{}@{}", x.utxo, x.value))
        // );

        let total_receiver = match dst {
            Direction::Issuer => {
                total_d_issuer = match &total_d_issuer {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value + contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 1)),
                };
                total_d_issuer.clone()
            }
            Direction::Owner => {
                total_d_owner = match &total_d_owner {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value + contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 1)),
                };
                total_d_owner.clone()
            }
            Direction::Other => {
                total_d_other = match &total_d_other {
                    ContractValue::Value(value) => {
                        ContractValue::Value(value + contract_amount.to_value())
                    }
                    _ => ContractValue::UDA(UDAValue::with(1, 1)),
                };
                total_d_other.clone()
            }
        };

        let contract = get_contract(&receiver_sk.to_string(), contract_id.to_string()).await?;
        assert_eq!(contract.balance, total_receiver);
        // println!(
        //     "Receiver: {:#?}",
        //     contract
        //         .allocations
        //         .into_iter()
        //         .map(|x| format!("{}@{}", x.utxo, x.value))
        // );
    }

    Ok(())
}
