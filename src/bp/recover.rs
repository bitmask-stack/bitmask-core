use crate::bitcoin::{BitcoinKeysError, save_mnemonic};
use crate::bp::{BtcAccountError, DrainOptions, drain_vault};
use crate::constants::{BTC_MAINNET_WPKH_PATH, BTC_TESTNET_WPKH_PATH, NETWORK, get_rgb_path};
use crate::structs::{FundVaultDetails, RecoverVaultDetails, SecretString, WatcherRequest};
use crate::wallet::watcher::{create_watcher, destroy_watcher, watcher_recover_utxos};
use bdk_wallet::keys::{DerivableKey, DescriptorKey, DescriptorSecretKey};
use bdk_wallet::miniscript::{Segwitv0, Tap};
use bip39::Mnemonic;
use bitcoin::bip32::{ChildNumber, DerivationPath, Fingerprint, Xpriv};
use bitcoin::secp256k1::{Keypair, Secp256k1};
use nostr::ToBech32;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const BTC_TAP_ASSETS_PATH: &str = "m/86h/0h/0h";
pub const BTC_TEST_TAP_ASSETS_PATH: &str = "m/86h/1h/0h";
// Default is P2TR (BIP-86) on m/86h/0h/0h (test m/86h/1h/0h). These
// account-20/21 paths would mint a second wallet. BIP-84 remains only for
// exchanges that still do not support P2TR. Not a Cargo feature.
// pub const RGB_TAP_ASSETS_PATH: &str = "m/86h/20h/0h";
// pub const RGB_TAP_UDAS_PATH: &str = "m/86h/21h/0h";

pub const RGB_ASSETS_PATH: &str = "m/168h/20h/0h";
pub const RGB_UDAS_PATH: &str = "m/168h/21h/0h";

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum RecoverError {
    /// invalid seed error. {0}
    InvalidSeed(String),
    /// invalid watcher error. {0}
    InvalidWatcher(String),
    /// transfer error. {0}
    Transfer(BtcAccountError),
}

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct PublicRecoverData {
    // Bitmask Now
    pub rgb_xpub: String,
    pub btc_xpub: String,
    pub btc_change_xpub: String,
    pub nostr_pub: String,
    pub nostr_npub: String,
    // Bitmask Mid 2024
    pub bitmask_mid24_xpub: String,
    pub bitmask_change_mid24_xpub: String,
    pub bitmask_rgb_mid24_xpub: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct PrivateRecoverData {
    pub rgb_xprv: String,
    pub btc_xprv: String,
    pub btc_change_xprv: String,
    pub nostr_prv: String,
    pub nostr_nsec: String,
    // Bitmask 2024
    pub bitmask_mid24_xprv: String,
    pub bitmask_rgb_mid24_xprv: String,
    pub bitmask_change_mid24_xprv: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct RecoverWalletData {
    pub mnemonic: String,
    pub private: PrivateRecoverData,
    pub public: PublicRecoverData,
}

pub async fn recover_funds_from_bad_wallets(
    seed: SecretString,
    pass_phrase: SecretString,
    recover_address: &str,
) -> Result<RecoverVaultDetails, RecoverError> {
    let bad_wallet = recover_bad_mnemonic(&seed, &pass_phrase)
        .await
        .map_err(|err| RecoverError::InvalidSeed(err.to_string()))?;
    let normal_wallet = save_mnemonic(&seed, &pass_phrase)
        .await
        .map_err(|err| RecoverError::InvalidSeed(err.to_string()))?;

    let PublicRecoverData {
        btc_xpub: ref bad_btc_xpub,
        rgb_xpub: ref bad_rgb_xpub,
        ref bitmask_mid24_xpub,
        ref bitmask_change_mid24_xpub,
        ..
    } = bad_wallet.public.clone();

    let PrivateRecoverData {
        btc_xprv: ref bad_btc_xpriv,
        btc_change_xprv: ref bad_btc_change_xpriv,
        ref bitmask_mid24_xprv,
        ref bitmask_change_mid24_xprv,
        ..
    } = bad_wallet.private.clone();

    let sk = &normal_wallet.private.nostr_prv;

    let mut utxos_recovered = Vec::new();
    let create_watch_req = WatcherRequest {
        name: "default".to_string(),
        btc_xpub: bitmask_mid24_xpub.to_string(),
        rgb_xpub: bitmask_change_mid24_xpub.to_string(),
        recover: true,
    };

    create_watcher(sk, create_watch_req.clone())
        .await
        .map_err(|err| RecoverError::InvalidWatcher(err.to_string()))?;

    let (descr, data) = watcher_recover_utxos(sk, "bitcoin")
        .await
        .map_err(|err| RecoverError::InvalidWatcher(err.to_string()))?;

    if !data.is_empty() {
        utxos_recovered.push((descr, data));
    }

    destroy_watcher(sk, true)
        .await
        .map_err(|err| RecoverError::InvalidWatcher(err.to_string()))?;

    // // Bitmask BAD Jul/25
    let create_watch_req = WatcherRequest {
        name: "default".to_string(),
        btc_xpub: bad_btc_xpub.to_string(),
        rgb_xpub: bad_rgb_xpub.to_string(),
        recover: true,
    };

    create_watcher(sk, create_watch_req.clone())
        .await
        .map_err(|err| RecoverError::InvalidWatcher(err.to_string()))?;

    let (descr, data) = watcher_recover_utxos(sk, "bitcoin")
        .await
        .map_err(|err| RecoverError::InvalidWatcher(err.to_string()))?;
    if !data.is_empty() {
        utxos_recovered.push((descr, data));
    }

    let (descr, data) = watcher_recover_utxos(sk, "rgb")
        .await
        .map_err(|err| RecoverError::InvalidWatcher(err.to_string()))?;
    if !data.is_empty() {
        utxos_recovered.push((descr, data));
    }

    destroy_watcher(sk, true)
        .await
        .map_err(|err| RecoverError::InvalidWatcher(err.to_string()))?;

    let secrets = vec![
        // Bad 25
        SecretString(bad_btc_xpriv.to_string()),
        SecretString(bad_btc_change_xpriv.to_string()),
        // Mid 24
        SecretString(bitmask_mid24_xprv.to_string()),
        SecretString(bitmask_change_mid24_xprv.to_string()),
    ];

    let mut options = DrainOptions::with(true, secrets);
    options.utxos = utxos_recovered.clone();

    let FundVaultDetails {
        fund_txid,
        fund_fee,
        ..
    } = drain_vault(sk, recover_address, options)
        .await
        .map_err(RecoverError::Transfer)?;

    let resp = RecoverVaultDetails {
        txid: fund_txid,
        fee: fund_fee,
    };

    Ok(resp)
}

pub async fn recover_bad_mnemonic(
    mnemonic_phrase: &SecretString,
    seed_password: &SecretString,
) -> Result<RecoverWalletData, BitcoinKeysError> {
    let mnemonic = Mnemonic::from_str(&mnemonic_phrase.0)?;
    get_bad_mnemonic(mnemonic, seed_password).await
}

pub async fn get_bad_mnemonic(
    mnemonic_phrase: Mnemonic,
    seed_password: &SecretString,
) -> Result<RecoverWalletData, BitcoinKeysError> {
    let seed = mnemonic_phrase.to_seed_normalized(&seed_password.0);

    let network = NETWORK.read().await;
    let xprv = Xpriv::new_master(*network, &seed)?;

    // Recover BIP-84 `wpkh` wallets. Default mint path is P2TR (BIP-86).
    // BIP-84 remains only for exchanges that still do not support P2TR.
    let btc_wpkh_path = if *network == bitcoin::Network::Bitcoin {
        BTC_MAINNET_WPKH_PATH
    } else {
        BTC_TESTNET_WPKH_PATH
    };
    let rgb_path = get_rgb_path().await;

    let btc_xprv = xprv_segwit_desc(&xprv, btc_wpkh_path, 0)?;
    let btc_xpub = xpub_segwit_desc(&xprv, btc_wpkh_path, 0)?;

    let btc_change_xprv = xprv_segwit_desc(&xprv, btc_wpkh_path, 1)?;
    let btc_change_xpub = xpub_segwit_desc(&xprv, btc_wpkh_path, 1)?;

    let rgb_xprv = xprv_desc(&xprv, &rgb_path, 10)?;
    let rgb_xpub = xpub_desc(&xprv, &rgb_path, 10)?;

    // Bitmask Mid 2023/2024
    let btc_tap_path = if *network == bitcoin::Network::Bitcoin {
        BTC_TAP_ASSETS_PATH
    } else {
        BTC_TEST_TAP_ASSETS_PATH
    };

    let bitmask_mid24_xpub = xpub_desc(&xprv, btc_tap_path, 0)?;
    let bitmask_change_mid24_xpub = xpub_desc(&xprv, btc_tap_path, 1)?;
    let bitmask_mid24_xprv = xprv_desc(&xprv, btc_tap_path, 0)?;
    let bitmask_change_mid24_xprv = xprv_desc(&xprv, btc_tap_path, 1)?;

    let bitmask_rgb_mid24_xpub = xpub_desc(&xprv, btc_tap_path, 10)?;
    let bitmask_rgb_mid24_xprv = xprv_desc(&xprv, btc_tap_path, 10)?;

    let (nostr_prv, nostr_pub) = nostr_keypair(&xprv)?;
    let nostr_keys = nostr::Keys::parse(&nostr_prv)?;
    let nostr_nsec = nostr_keys
        .secret_key()
        .to_bech32()
        .expect("nsec bech32 encode is infallible");
    let nostr_npub = nostr_keys
        .public_key()
        .to_bech32()
        .expect("npub bech32 encode is infallible");

    let private = PrivateRecoverData {
        rgb_xprv,
        btc_xprv,
        btc_change_xprv,
        nostr_prv,
        nostr_nsec,
        bitmask_mid24_xprv,
        bitmask_rgb_mid24_xprv,
        bitmask_change_mid24_xprv,
    };

    let public = PublicRecoverData {
        nostr_pub,
        nostr_npub,
        rgb_xpub,
        btc_xpub,
        btc_change_xpub,
        bitmask_mid24_xpub,
        bitmask_change_mid24_xpub,
        bitmask_rgb_mid24_xpub,
    };

    Ok(RecoverWalletData {
        mnemonic: mnemonic_phrase.to_string(),
        private,
        public,
    })
}

fn xprv_desc(xprv: &Xpriv, path: &str, change: u32) -> Result<String, BitcoinKeysError> {
    let xprv = get_descriptor(xprv, path, change)?;
    let desc = format!("tr({xprv})");

    Ok(desc)
}

fn xpub_desc(xprv: &Xpriv, path: &str, change: u32) -> Result<String, BitcoinKeysError> {
    let secp = Secp256k1::new();
    let xprv = get_descriptor(xprv, path, change)?;
    let xpub = xprv.to_public(&secp)?;
    let desc = format!("tr({xpub})");
    Ok(desc)
}

fn xprv_segwit_desc(xprv: &Xpriv, path: &str, change: u32) -> Result<String, BitcoinKeysError> {
    let xprv = get_segwit_descriptor(xprv, path, change)?;
    let desc = format!("wpkh({xprv})");
    Ok(desc)
}

fn xpub_segwit_desc(xprv: &Xpriv, path: &str, change: u32) -> Result<String, BitcoinKeysError> {
    let secp = Secp256k1::new();
    let xprv = get_segwit_descriptor(xprv, path, change)?;
    let xpub = xprv.to_public(&secp)?;
    let desc = format!("wpkh({xpub})");
    Ok(desc)
}

// For NIP-06 Nostr signing and Carbonado encryption key derivation
fn nostr_keypair(xprv: &Xpriv) -> Result<(String, String), BitcoinKeysError> {
    const NOSTR_PATH: &str = "m/44'/1237'/0'/0/0";
    let deriv_descriptor = DerivationPath::from_str(NOSTR_PATH)?;
    let secp = Secp256k1::new();
    let nostr_sk = xprv.derive_priv(&secp, &deriv_descriptor)?;
    let keypair =
        Keypair::from_seckey_slice(&secp, nostr_sk.private_key.secret_bytes().as_slice())?;

    Ok((
        hex::encode(nostr_sk.private_key.secret_bytes()),
        hex::encode(keypair.x_only_public_key().0.serialize()),
    ))
}

fn get_descriptor(
    xprv: &Xpriv,
    path: &str,
    change: u32,
) -> Result<DescriptorSecretKey, BitcoinKeysError> {
    let secp = Secp256k1::default();
    let deriv_descriptor = DerivationPath::from_str(path)?;
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor)?;
    let origin: (Fingerprint, DerivationPath) = (xprv.fingerprint(&secp), deriv_descriptor);

    let derived_xprv_desc_key: DescriptorKey<Tap> = derived_xprv.into_descriptor_key(
        Some(origin),
        DerivationPath::default().child(ChildNumber::from_normal_idx(change)?),
    )?;

    if let DescriptorKey::Secret(desc_seckey, _, _) = derived_xprv_desc_key {
        Ok(desc_seckey)
    } else {
        Err(BitcoinKeysError::UnexpectedKey)
    }
}

fn get_segwit_descriptor(
    xprv: &Xpriv,
    path: &str,
    change: u32,
) -> Result<DescriptorSecretKey, BitcoinKeysError> {
    let secp = Secp256k1::new();
    let deriv_descriptor = DerivationPath::from_str(path)?;
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor)?;
    let origin: (Fingerprint, DerivationPath) = (xprv.fingerprint(&secp), deriv_descriptor);

    let derived_xprv_desc_key: DescriptorKey<Segwitv0> = derived_xprv.into_descriptor_key(
        Some(origin),
        DerivationPath::default().child(ChildNumber::from_normal_idx(change)?),
    )?;

    if let DescriptorKey::Secret(desc_seckey, _, _) = derived_xprv_desc_key {
        Ok(desc_seckey)
    } else {
        Err(BitcoinKeysError::UnexpectedKey)
    }
}
