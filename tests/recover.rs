#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use bitmask_core::bp::recover::recover_bad_mnemonic;
use bitmask_core::constants::switch_network;
use bitmask_core::structs::SecretString;

/// Recover of old BIP-84 wallets: on bitcoin, drain xprv and watcher xpub
/// share `m/84h/0h/0h`. They must not mix mainnet `84'/0'/0'` with the
/// always-testnet `bad_btc_path` (`84'/1'/0'`).
#[tokio::test]
async fn get_bad_mnemonic_bitcoin_wpkh_xprv_xpub_share_bip84_account() -> Result<()> {
    switch_network("bitcoin").await?;

    const MNEMONIC: &str = "empty faculty salute fortune select asthma attract question violin movie smile erupt half step lion deposit render stumble double mobile fossil height usual topple";

    let wallet = recover_bad_mnemonic(
        &SecretString(MNEMONIC.to_owned()),
        &SecretString(String::new()),
    )
    .await?;

    assert!(
        wallet.private.btc_xprv.contains("84'/0'/0'"),
        "recover xprv uses bitcoin BIP-84 account, got {}",
        wallet.private.btc_xprv
    );
    assert!(
        wallet.public.btc_xpub.contains("84'/0'/0'"),
        "recover xpub uses bitcoin BIP-84 account, got {}",
        wallet.public.btc_xpub
    );

    Ok(())
}
