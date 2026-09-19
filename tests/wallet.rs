#![cfg(not(target_arch = "wasm32"))]
use std::env;

use anyhow::Result;
use bitmask_core::{
    bitcoin::{
        BitcoinError, bump_fee, decrypt_wallet, encrypt_wallet, get_wallet_data, hash_password,
        new_wallet, send_sats,
    },
    constants::{get_network, switch_network},
    structs::SecretString,
    util::init_logging,
    warn,
};
use log::info;

const ENCRYPTION_PASSWORD: &str = "hunter2";
const SEED_PASSWORD: &str = "";

#[tokio::test]
async fn error_for_bad_mnemonic() -> Result<()> {
    init_logging("wallet=info");

    let network = get_network().await;
    info!("Wallet test on {network}");

    info!("Import wallets");
    let mnemonic = "this is a bad mnemonic that is meant to break";
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()))?;
    let mnemonic_data_result = encrypt_wallet(
        &SecretString(mnemonic.to_owned()),
        &hash,
        &SecretString(SEED_PASSWORD.to_owned()),
    )
    .await;

    assert!(mnemonic_data_result.is_err());

    Ok(())
}

/// Hermetic encrypt then decrypt of descriptors. Wire format is DIBA magic +
/// version byte + 24-byte XChaCha20 nonce + bincode ciphertext (the old
/// serde-encrypt SharedKey layout). Do not weaken this if the AEAD crate
/// changes.
#[tokio::test]
async fn encrypt_decrypt_wallet_descriptors_roundtrip() -> Result<()> {
    init_logging("wallet=info");

    switch_network("bitcoin").await?;

    const MNEMONIC: &str = "empty faculty salute fortune select asthma attract question violin movie smile erupt half step lion deposit render stumble double mobile fossil height usual topple";
    const ENCRYPTION_PASSWORD: &str = "hunter2";

    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()))?;
    let encrypted = encrypt_wallet(
        &SecretString(MNEMONIC.to_owned()),
        &hash,
        &SecretString(SEED_PASSWORD.to_owned()),
    )
    .await?;

    // DIBA || version 0, then 24-byte nonce, then ciphertext. Hex-encoded.
    assert!(
        encrypted.0.len() > (5 + 24) * 2,
        "versioned descriptor is magic + version + nonce + ciphertext"
    );
    assert!(
        encrypted.0.starts_with("4449424100"),
        "DIBA magic prefix and version byte 0"
    );

    let decrypted = decrypt_wallet(&hash, &encrypted, &SecretString("".to_owned())).await?;
    assert_eq!(decrypted.mnemonic, MNEMONIC, "mnemonic roundtrips");
    // Default is P2TR (BIP-86). BIP-84 remains only for exchanges that still
    // do not support P2TR. Same mnemonic and `tr`/`86` string as
    // `tests/keys.rs::taproot`.
    // wpkh([496f1ccc/84'/0'/0']xpub6BkBoEEqTfx9zjBT4gmFHYiZktNCL7G5cExmf1g1ZyP1jZp6rn4XQeK6Hvhr1FTVnEZSVjYuNmhFiw31hECc8QqE7Vw27Azs6smrE6Tqfv4/0/*)
    assert_eq!(
        decrypted.public.btc_descriptor_xpub,
        "tr([496f1ccc/86'/0'/0']xpub6CBkARCPxmbRjaxzHxC38e9sKUVtMTRFqBYUFdXAHFBpeQzJz6mYSaQ1qSvCrNzYUNuvpD9FS6fmK9YowdCxaiCUSpjzNm5hvV2JxEodZ1q/0/*)",
        "BIP-86 taproot xpub descriptor roundtrips"
    );

    Ok(())
}

#[tokio::test]
async fn create_wallet() -> Result<()> {
    init_logging("wallet=info");

    switch_network("bitcoin").await?;
    let network = get_network().await;
    info!("Test on {network}");

    info!("Create wallet");
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()))?;
    let encrypted_descriptors = new_wallet(&hash, &SecretString(SEED_PASSWORD.to_owned())).await?;
    let decrypted_wallet =
        decrypt_wallet(&hash, &encrypted_descriptors, &SecretString("".to_owned())).await?;

    let btc_wallet = get_wallet_data(
        &SecretString(decrypted_wallet.private.btc_descriptor_xprv.clone()),
        None,
    )
    .await?;

    println!(
        "Descriptor: {}",
        decrypted_wallet.private.btc_descriptor_xprv
    );
    println!("Address ({network}): {}", btc_wallet.address);

    switch_network("regtest").await?;
    let network = get_network().await;
    info!("Test on {network}");

    info!("Create wallet");
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()))?;
    let encrypted_descriptors = new_wallet(&hash, &SecretString(SEED_PASSWORD.to_owned())).await?;
    let decrypted_wallet =
        decrypt_wallet(&hash, &encrypted_descriptors, &SecretString("".to_owned())).await?;

    let btc_wallet = get_wallet_data(
        &SecretString(decrypted_wallet.private.btc_descriptor_xprv.clone()),
        None,
    )
    .await?;

    println!(
        "Descriptor: {}",
        decrypted_wallet.private.btc_descriptor_xprv
    );
    println!("Address ({network}): {}", btc_wallet.address);

    Ok(())
}

#[tokio::test]
async fn import_wallet() -> Result<()> {
    init_logging("wallet=info");

    let network = get_network().await;
    info!("Asset test on {network}");

    info!("Import wallets");
    let seed_password = SecretString(SEED_PASSWORD.to_owned());
    let main_mnemonic = SecretString(env::var("TEST_WALLET_SEED")?);
    let hash0 = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()))?;
    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &hash0, &seed_password).await?;
    let _main_vault =
        decrypt_wallet(&hash0, &encrypted_descriptors, &SecretString("".to_owned())).await?;

    info!("Try once more");
    let hash1 = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()))?;
    assert_eq!(hash0.0, hash1.0, "hashes match");

    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &hash1, &seed_password).await?;
    let main_vault =
        decrypt_wallet(&hash1, &encrypted_descriptors, &SecretString("".to_owned())).await?;

    let main_btc_wallet = get_wallet_data(
        &SecretString(main_vault.private.btc_descriptor_xprv.clone()),
        None,
    )
    .await?;
    let main_rgb_wallet = get_wallet_data(
        &SecretString(main_vault.private.rgb_descriptor_xprv.clone()),
        None,
    )
    .await?;

    println!("Descriptor: {}", main_vault.private.btc_descriptor_xprv);
    println!("Address (Bitcoin): {}", main_btc_wallet.address);
    println!("Address (RGB): {}", main_rgb_wallet.address);

    Ok(())
}

#[tokio::test]
async fn get_wallet_balance() -> Result<()> {
    init_logging("wallet=info");

    let main_mnemonic = SecretString(env::var("TEST_WALLET_SEED")?);
    let seed_password = SecretString(SEED_PASSWORD.to_owned());
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()))?;
    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &hash, &seed_password).await?;
    let main_vault =
        decrypt_wallet(&hash, &encrypted_descriptors, &SecretString("".to_owned())).await?;

    let btc_wallet = get_wallet_data(
        &SecretString(main_vault.private.btc_descriptor_xprv.clone()),
        None,
    )
    .await?;

    warn!("Descriptor:", main_vault.private.btc_descriptor_xprv);
    warn!("Address:", btc_wallet.address);
    warn!("Wallet Balance:", btc_wallet.balance.confirmed.to_string());

    Ok(())
}

#[tokio::test]
async fn wrong_network() -> Result<()> {
    init_logging("wallet=info");

    switch_network("testnet").await?;
    let network = get_network().await;
    info!("Asset test on {network}");

    let main_mnemonic = SecretString(env::var("TEST_WALLET_SEED")?);
    let seed_password = SecretString(SEED_PASSWORD.to_owned());
    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()))?;
    let encrypted_descriptors = encrypt_wallet(&main_mnemonic, &hash, &seed_password).await?;

    let main_vault =
        decrypt_wallet(&hash, &encrypted_descriptors, &SecretString("".to_owned())).await?;

    let result = send_sats(
        &SecretString(main_vault.private.btc_descriptor_xprv.to_owned()),
        &SecretString(main_vault.private.btc_change_descriptor_xprv.to_owned()),
        "bc1pgxpvg7cz0s3akgl9vhv687rzya7frskenukgx3gwuh6q3un5wqgq7xmnhe",
        1000,
        true,
        Some(1.0),
    )
    .await;

    assert!(matches!(result, Err(BitcoinError::WrongNetwork)));

    Ok(())
}

/// Unknown txid with `broadcast: false` does not sync Esplora. `build_fee_bump`
/// on a wallet that never saw the tx is `BdkFeeBumpError`. Name is not
/// `create_wallet` so Nix nextest runs it.
#[tokio::test]
async fn bump_fee_unknown_txid_is_bdk_fee_bump_error() -> Result<()> {
    init_logging("wallet=info");

    switch_network("bitcoin").await?;

    const MNEMONIC: &str = "empty faculty salute fortune select asthma attract question violin movie smile erupt half step lion deposit render stumble double mobile fossil height usual topple";

    let hash = hash_password(&SecretString(ENCRYPTION_PASSWORD.to_owned()))?;
    let encrypted = encrypt_wallet(
        &SecretString(MNEMONIC.to_owned()),
        &hash,
        &SecretString(SEED_PASSWORD.to_owned()),
    )
    .await?;

    let decrypted = decrypt_wallet(&hash, &encrypted, &SecretString("".to_owned())).await?;

    let txid = "0000000000000000000000000000000000000000000000000000000000000001";
    let err = bump_fee(
        txid.into(),
        2.0,
        &SecretString(decrypted.private.btc_descriptor_xprv.clone()),
        Some(&SecretString(
            decrypted.private.btc_change_descriptor_xprv.clone(),
        )),
        false,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, BitcoinError::BdkFeeBumpError(_)));

    Ok(())
}
