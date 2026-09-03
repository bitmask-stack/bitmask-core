#![cfg(not(target_arch = "wasm32"))]
use crate::rgb::server::seeder::{FundSeed, create_new_fund_wallet, get_uri_media};
use bitmask_core::constants::get_server_wallet_seed;
use bitmask_core::regtest::{new_block, send_coins};
use bitmask_core::rgb::issue::IssueContractOptions;
use bitmask_core::rgb::swap::BidOptions;
use bitmask_core::rgb::{
    atomic_mint, atomic_refresh, atomic_stats, bids, create_bid, get_contract_media,
    verify_transfers,
};
use bitmask_core::structs::{
    AtomicMintRequest, ChainFee, OrderValue, PrivateWalletData, RgbBidRequest, RgbOfferResponse,
    SecretString,
};

#[tokio::test]
async fn create_atomic_mint() -> anyhow::Result<()> {
    // Setup
    let server_keys = create_new_fund_wallet(FundSeed::new(
        get_server_wallet_seed().await,
        "1.00000000".to_string(),
    ))
    .await?;
    let owner_keys = create_new_fund_wallet(FundSeed::default()).await?;

    // Atomic Mint
    let options = IssueContractOptions {
        proxy: true,
        ..Default::default()
    };

    let req = AtomicMintRequest {
        ticker: "ATMI1".to_string(),
        name: "ATMI1".to_string(),
        iface: "RGB21".to_string(),
        description: "ATMI1".to_string(),
        chain: Default::default(),
        meta: Some(get_uri_media()),
        bitcoin_price: 1000,
    };

    let resp = atomic_mint(req, options).await?;

    let RgbOfferResponse { id, contract, .. } = resp.rgb_offer_response[0].clone();

    let contract_id = contract.info.contract_id();
    let contract_exported =
        get_contract_media(&server_keys.private.nostr_prv, contract_id.to_string()).await?;
    assert!(contract_exported.meta.is_some());

    let req = RgbBidRequest {
        offer_id: id.to_string(),
        asset_price: OrderValue::bitcoin(1000),
        fee: ChainFee::Value(1000),
        bundle_id: None,
    };

    let PrivateWalletData {
        nostr_prv: ref owner_sk,
        btc_descriptor_xprv: ref owner_btc_prv,
        btc_change_descriptor_xprv: ref owner_btc_change_prv,
        rgb_descriptor_xprv: ref owner_rgb_prv,
        ..
    } = owner_keys.private.clone();

    let secrets = vec![
        SecretString(owner_rgb_prv.to_string()),
        SecretString(owner_btc_prv.to_string()),
        SecretString(owner_btc_change_prv.to_string()),
    ];

    let options = BidOptions::p2p(secrets);
    let new_bid = create_bid(&owner_sk.to_string(), req, options).await?;
    new_block();

    let my_bids = bids(&owner_sk.to_string()).await?;
    assert!(my_bids.iter().any(|x| x.id == new_bid.id));

    verify_transfers(&owner_sk.to_string(), true).await?;
    atomic_refresh().await?;

    let stats_data = atomic_stats().await?;
    let address = stats_data.addresses.last().unwrap();
    send_coins(address, "0.45");
    new_block();

    atomic_refresh().await?;
    let _ = atomic_stats().await?;
    // println!("\n{}", format!("Stats: \n{:#?}", stats_data));

    Ok(())
}
