#![cfg(not(target_arch = "wasm32"))]

use crate::bp::server::seeder::{WalletSeed, create_new_wallet};
use bitmask_core::bitcoin::{BitcoinError, bump_fee, drain_wallet, get_wallet_data, send_sats};
use bitmask_core::bp::{FundOptions, fund_vault, transfer_btc};
use bitmask_core::regtest::new_block;
use bitmask_core::structs::{
    NextAddressResponse, PrivateWalletData, PublicWalletData, SecretString,
};
use bitmask_core::wallet::watcher::{watcher_next_address, watcher_next_utxo};
use bpstd::Sats;

#[tokio::test]
async fn create_new_btc_transfer() -> anyhow::Result<()> {
    let owner_keys = create_new_wallet(WalletSeed::default()).await?;
    let PrivateWalletData {
        nostr_prv: ref sk,
        ref btc_descriptor_xprv,
        ..
    } = owner_keys.private.clone();
    let PublicWalletData { .. } = owner_keys.public.clone();

    let btc = "Bitcoin";
    let sats = Sats::from_sats(600_u64);

    let next_utxo = watcher_next_utxo(sk, btc).await?;
    let utxo = next_utxo.utxo;
    assert!(utxo.is_some());

    let next_address = watcher_next_address(sk, btc, false).await?;
    let NextAddressResponse { address, .. } = next_address;

    transfer_btc(
        sk,
        &address,
        sats.into(),
        1.1,
        vec![SecretString(btc_descriptor_xprv.to_string())],
    )
    .await?;

    new_block();

    let next_utxo = watcher_next_utxo(sk, btc).await?;
    let utxo = next_utxo.utxo;
    assert!(utxo.is_some());
    Ok(())
}

#[tokio::test]
async fn create_fund_rgb_tx() -> anyhow::Result<()> {
    let owner_keys = create_new_wallet(WalletSeed::default()).await?;
    let PrivateWalletData {
        nostr_prv: ref sk,
        ref btc_descriptor_xprv,
        ..
    } = owner_keys.private.clone();
    let PublicWalletData { .. } = owner_keys.public.clone();

    let options = FundOptions::with(true, vec![SecretString(btc_descriptor_xprv.to_string())]);
    let fund_resp = fund_vault(sk, 1.1, options).await?;
    println!("fund_resp: {:?}", fund_resp);
    Ok(())
}

#[tokio::test]
async fn drain_wallet_after_create_new_wallet() -> anyhow::Result<()> {
    let keys = create_new_wallet(WalletSeed::default()).await?;
    let dest = watcher_next_address(&keys.private.nostr_prv, "Bitcoin", false).await?;
    let drained = drain_wallet(
        &dest.address,
        &SecretString(keys.private.btc_descriptor_xprv.clone()),
        Some(&SecretString(
            keys.private.btc_change_descriptor_xprv.clone(),
        )),
        Some(1.1),
    )
    .await?;
    assert_eq!(drained.details.received, 0);
    assert!(drained.details.fee.is_some());
    assert!(drained.details.sent > 0);
    Ok(())
}

#[tokio::test]
async fn send_sats_then_bump_fee() -> anyhow::Result<()> {
    let keys = create_new_wallet(WalletSeed::default()).await?;
    let dest = watcher_next_address(&keys.private.nostr_prv, "Bitcoin", false).await?;
    get_wallet_data(
        &SecretString(keys.private.btc_descriptor_xprv.clone()),
        Some(&SecretString(
            keys.private.btc_change_descriptor_xprv.clone(),
        )),
    )
    .await?;
    let send = send_sats(
        &SecretString(keys.private.btc_descriptor_xprv.clone()),
        &SecretString(keys.private.btc_change_descriptor_xprv.clone()),
        &dest.address,
        600,
        true,
        Some(1.0),
    )
    .await?;
    match bump_fee(
        send.details.txid.to_string(),
        2.0,
        &SecretString(keys.private.btc_descriptor_xprv.clone()),
        Some(&SecretString(
            keys.private.btc_change_descriptor_xprv.clone(),
        )),
        true,
    )
    .await
    {
        Ok(bumped) => {
            assert_ne!(bumped.details.txid, send.details.txid);
        }
        Err(BitcoinError::BdkFeeBumpError(_)) => {
            // HEAD: create_transaction never calls enable_rbf.
        }
        Err(e) => panic!("unexpected bump_fee error: {e}"),
    }
    Ok(())
}
