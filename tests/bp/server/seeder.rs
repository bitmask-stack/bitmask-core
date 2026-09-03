#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(not(target_arch = "wasm32"))]

use amplify::{default, none};
use bitmask_core::bitcoin::{get_mnemonic, get_wallet_data, new_mnemonic, save_mnemonic};
use bitmask_core::bp::{FundOptions, fund_vault};
use bitmask_core::regtest::{new_block, send_coins};
use bitmask_core::structs::{
    ChainFee, ChainTarget, DecryptedWalletData, FundVaultDetails, NextAddressResponse,
    NextUtxoResponse, PrivateWalletData, PublicWalletData, SecretString, WatcherRequest,
    WatcherResponse,
};
use bitmask_core::wallet::watcher::{
    create_watcher, watcher_next_address, watcher_next_change_address,
};
use bitmask_core::{debug, info};
use std::str::FromStr;

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

impl WalletSeed {
    pub fn with(mnemonic: String, amount: String) -> Self {
        Self {
            mnemonic: Some(mnemonic),
            amount: Some(amount),
        }
    }
}

pub async fn create_new_wallet(seed: WalletSeed) -> Result<DecryptedWalletData, anyhow::Error> {
    let keys = new_mnemonic(&SecretString("".to_string())).await?;

    let iface = "Bitcoin";
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

    let watcher_resp = create_watcher(sk, create_watch_req.clone()).await?;

    let next_address = watcher_next_address(sk, iface, false).await?;
    let NextAddressResponse {
        address, keychain, ..
    } = next_address;

    println!("Fund Wallet: {address}/{keychain}");

    let amount = seed.amount.map_or("1".to_string(), |x| x.to_string());
    send_coins(&address, &amount);

    Ok(keys)
}

pub async fn create_new_fund_wallet(
    seed: WalletSeed,
) -> Result<DecryptedWalletData, anyhow::Error> {
    let keys = if let Some(seed) = seed.mnemonic {
        save_mnemonic(&SecretString(seed), &SecretString("".to_string())).await?
    } else {
        new_mnemonic(&SecretString("".to_string())).await?
    };

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

    let watcher_resp = create_watcher(sk, create_watch_req.clone()).await?;

    let next_address = watcher_next_address(sk, btc, true).await?;
    let amount = seed.amount.map_or("1".to_string(), |x| x.to_string());
    send_coins(&next_address.address, &amount);
    new_block();

    let options = FundOptions::with(
        true,
        vec![
            SecretString(btc_descriptor_xprv.to_string()),
            SecretString(btc_change_descriptor_xprv.to_string()),
        ],
    );
    let resp = fund_vault(sk, 1.1, options).await?;

    new_block();

    assert!(resp.rgb_output.is_some());
    Ok(keys)
}
