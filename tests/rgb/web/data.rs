#![cfg(target_arch = "wasm32")]
use std::str::FromStr;

use crate::rgb::web::seeder::{
    SeedTransfer, WalletSeed, create_and_publish_transfer, create_new_wallet,
};
use crate::rgb::web::utils::{USER_PUBKEY, new_block};
use bitmask_core::bitcoin::new_mnemonic;
use bitmask_core::info;
use bitmask_core::rgb::constants::RGB_DATA_VERSION;
use bitmask_core::rgb::data::{RGBStock, RGBStockCrossStore};
use bitmask_core::rgb::models::{AssetType, ContractAmount};
use bitmask_core::structs::{
    ContractResponse, ContractValue, PrivateWalletData, PublicWalletData, RgbInvoiceValue,
    RgbTransferResponse, SecretString, WatcherWalletResponse,
};
use bitmask_core::wallet::data::{AccountCrossStore, BPWallet, CrossStoreAsync, SyncProvider};
use bitmask_core::web::rgb::{
    backup_rgb_data, get_rgb_wallet, list_contracts, restore_rgb_data, verify_transfers,
};
use bitmask_core::web::wallet::destroy_watcher;
use bitmask_core::web::{json_parse, resolve};
use bpstd::{HardenedIndex, Xpub};
use bpwallet::{DerivationPath, Network, XkeyOrigin, XpubDerivable};
use rgb::{RgbDescr, TapretKey};
use rgbstd::containers::Kit;
use rgbstd::contract::IssuerWrapper;
use rgbstd::persistence::Stock;
use schemata::CollectibleFungibleAsset;
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// #[wasm_bindgen_test]
#[ignore = "we remove local-first support"]
async fn create_or_open_rgb_stock() -> anyhow::Result<()> {
    let network = Network::Regtest;
    let keys = new_mnemonic(&SecretString("".to_string())).await?;

    let sk = keys.private.nostr_prv.clone();

    let schema = CollectibleFungibleAsset::schema();
    let lib = CollectibleFungibleAsset::scripts();
    let types = CollectibleFungibleAsset::types();

    let mut kit = Kit::default();
    kit.schemata.push(schema)?;
    kit.scripts.extend(lib.into_values())?;
    kit.types = types;

    let mut rgb_stock = Stock::in_memory();
    rgb_stock
        .import_kit(kit.validate().unwrap())
        .expect("invalid import kit");

    let mut provider = RGBStockCrossStore::with(&sk, &network.to_string())?;
    rgb_stock.sync(&mut provider).expect("invalid stock store");
    provider.persist().await?;

    let mut provider2 = RGBStockCrossStore::with(&sk, &network.to_string())?;
    provider2.fetch().await?;

    let _rgb_stock_2 = RGBStock::get(provider2).expect("invalid stock store");

    Ok(())
}

#[wasm_bindgen_test]
async fn create_new_rgb_wallet_backup_restore() -> anyhow::Result<()> {
    let network = Network::Regtest;
    let keys = new_mnemonic(&SecretString("".to_string())).await?;

    let account: u8 = network.is_testnet().into();
    let account = format!("86h/{account}h/0h");
    let derivation: DerivationPath<HardenedIndex> = account.parse()?;

    let xpub = USER_PUBKEY.to_string();
    let xpub = Xpub::from_str(&xpub)?;
    let fp = xpub.fingerprint();

    let origin = XkeyOrigin::new(fp, derivation);

    let desc = format!("[{origin}]{xpub}/<0;1;9;10>/*");
    let xpub = XpubDerivable::from_str(&desc)?;
    let taproot = TapretKey::new_unfunded(xpub);
    let descr = RgbDescr::from(taproot);

    let mut rgb_wallet = BPWallet::new_layer1(descr, network);
    let sk = keys.private.nostr_prv.clone();

    let mut provider =
        AccountCrossStore::with(&sk, &network.to_string(), &RGB_DATA_VERSION, false)?;
    rgb_wallet.sync(&mut provider).expect("");
    provider.persist().await?;

    let schema = CollectibleFungibleAsset::schema();
    let lib = CollectibleFungibleAsset::scripts();
    let types = CollectibleFungibleAsset::types();

    let mut kit = Kit::default();
    kit.schemata.push(schema)?;
    kit.scripts.extend(lib.into_values())?;
    kit.types = types;

    let mut rgb_stock = Stock::in_memory();
    rgb_stock
        .import_kit(kit.validate().unwrap())
        .expect("invalid import kit");

    let mut provider = RGBStockCrossStore::with(&sk, &network.to_string())?;
    rgb_stock.sync(&mut provider).expect("invalid stock store");
    provider.persist().await?;

    let backup = resolve(backup_rgb_data(sk.to_string())).await;
    let backup: bool = json_parse(&backup);
    assert!(backup);

    let restore = resolve(restore_rgb_data(sk.to_string())).await;
    let restore: bool = json_parse(&restore);
    assert!(restore);
    Ok(())
}

#[wasm_bindgen_test]
async fn create_restore_from_used_wallets() -> anyhow::Result<()> {
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
    let RgbTransferResponse { .. } = create_and_publish_transfer(seed).await?;

    info!("Create new Block");
    let _ = new_block().await;

    resolve(verify_transfers(sender_sk.to_string())).await;
    resolve(verify_transfers(beneficiary_sk.to_string())).await;

    let contract_resp = resolve(list_contracts(sender_sk.to_string(), true)).await;
    let contract_resp: Vec<ContractResponse> = json_parse(&contract_resp);
    let contract_resp = contract_resp[0].clone();
    assert_eq!(contract_resp.balance, ContractValue::Value(410));

    let contract_resp = resolve(list_contracts(beneficiary_sk.to_string(), true)).await;
    let contract_resp: Vec<ContractResponse> = json_parse(&contract_resp);
    let contract_resp = contract_resp[0].clone();
    assert_eq!(contract_resp.balance, ContractValue::Value(100));

    info!("Backup Wallets");
    resolve(backup_rgb_data(sender_sk.to_string())).await;
    resolve(backup_rgb_data(beneficiary_sk.to_string())).await;

    info!("Destroy Wallets");
    resolve(destroy_watcher(sender_sk.to_string())).await;
    resolve(destroy_watcher(beneficiary_sk.to_string())).await;

    info!("Restore Wallets");
    resolve(restore_rgb_data(sender_sk.to_string())).await;
    resolve(restore_rgb_data(beneficiary_sk.to_string())).await;

    let contract_resp = resolve(list_contracts(sender_sk.to_string(), true)).await;
    let contract_resp: Vec<ContractResponse> = json_parse(&contract_resp);
    let contract_resp = contract_resp[0].clone();
    assert_eq!(contract_resp.balance, ContractValue::Value(410));

    let contract_resp = resolve(list_contracts(beneficiary_sk.to_string(), true)).await;
    let contract_resp: Vec<ContractResponse> = json_parse(&contract_resp);
    let contract_resp = contract_resp[0].clone();
    assert_eq!(contract_resp.balance, ContractValue::Value(100));

    let rgb_wallet_resp = resolve(get_rgb_wallet(sender_sk.to_string())).await;
    let rgb_wallet: WatcherWalletResponse = json_parse(&rgb_wallet_resp);
    assert!(rgb_wallet.next_utxo.is_some());

    Ok(())
}
