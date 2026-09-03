use bdk_wallet::bitcoin::{
    bip32::{ChildNumber, DerivationPath, Fingerprint, Xpriv, Xpub},
    secp256k1::{Keypair, Secp256k1},
};
use bdk_wallet::keys::{
    DerivableKey, DescriptorKey, DescriptorKey::Secret as SecretDesc, DescriptorSecretKey, KeyError,
};
use bdk_wallet::miniscript::Tap;
use bdk_wallet::miniscript::descriptor::DescriptorKeyParseError;
// Default is P2TR (BIP-86). BIP-84 remains only for exchanges that still do
// not support P2TR. Not a Cargo feature.
// use bdk_wallet::miniscript::Segwitv0;
use bip39::{Language, Mnemonic};
use bitcoin::hashes::{Hash, sha256};
use nostr::ToBech32;
use std::str::FromStr;
use thiserror::Error;
use zeroize::Zeroize;

use crate::constants::{get_btc_path, get_rgb_path};
use crate::{
    constants::NETWORK,
    structs::{DecryptedWalletData, PrivateWalletData, PublicWalletData, SecretString},
};

type KeySource = (Fingerprint, DerivationPath);

#[allow(clippy::result_large_err)] // descriptor key error graph; do not reshape
#[derive(Error, Debug)]
pub enum BitcoinKeysError {
    /// Unexpected key variant in get_descriptor
    #[error("Unexpected key variant in get_descriptor")]
    UnexpectedKey,
    /// Unexpected xpub descriptor
    #[error("Unexpected xpub descriptor")]
    UnexpectedWatcherXpubDescriptor,
    /// Unexpected key variant in nostr_keypair
    #[error("Unexpected key variant in nostr_keypair")]
    UnexpectedKeyVariantInNostrKeypair,
    /// secp256k1 error
    #[error(transparent)]
    Secp256k1Error(#[from] bitcoin::secp256k1::Error),
    /// BIP-32 error
    #[error(transparent)]
    Bip32Error(#[from] bitcoin::bip32::Error),
    /// BIP-39 error
    #[error(transparent)]
    Bip39Error(#[from] bip39::Error),
    /// BDK key error
    #[error(transparent)]
    BdkKeyError(#[from] KeyError),
    /// Miniscript descriptor key parse error
    #[error(transparent)]
    MiniscriptDescriptorKeyParseError(#[from] DescriptorKeyParseError),
    /// getrandom error
    #[error(transparent)]
    GetRandomError(#[from] getrandom_02::Error),
    /// Nostr key parse error
    #[error(transparent)]
    NostrKeyError(#[from] nostr::key::Error),
}

fn get_descriptor(
    xprv: &Xpriv,
    path: &str,
    change: u32,
) -> Result<DescriptorSecretKey, BitcoinKeysError> {
    let secp = Secp256k1::new();
    let deriv_descriptor = DerivationPath::from_str(path)?;
    let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor)?;
    let origin: KeySource = (xprv.fingerprint(&secp), deriv_descriptor);

    let derived_xprv_desc_key: DescriptorKey<Tap> = derived_xprv.into_descriptor_key(
        Some(origin),
        DerivationPath::default().child(ChildNumber::from_normal_idx(change)?),
    )?;

    if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
        Ok(desc_seckey)
    } else {
        Err(BitcoinKeysError::UnexpectedKey)
    }
}

// Default is P2TR (BIP-86). BIP-84 remains only for exchanges that still do
// not support P2TR. Not a Cargo feature.
// fn get_segwit_descriptor(
//     xprv: &Xpriv,
//     path: &str,
//     change: u32,
// ) -> Result<DescriptorSecretKey, BitcoinKeysError> {
//     let secp = Secp256k1::new();
//     let deriv_descriptor = DerivationPath::from_str(path)?;
//     let derived_xprv = &xprv.derive_priv(&secp, &deriv_descriptor)?;
//     let origin: KeySource = (xprv.fingerprint(&secp), deriv_descriptor);
//
//     let derived_xprv_desc_key: DescriptorKey<Segwitv0> = derived_xprv.into_descriptor_key(
//         Some(origin),
//         DerivationPath::default().child(ChildNumber::from_normal_idx(change)?),
//     )?;
//
//     if let SecretDesc(desc_seckey, _, _) = derived_xprv_desc_key {
//         Ok(desc_seckey)
//     } else {
//         Err(BitcoinKeysError::UnexpectedKey)
//     }
// }

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

// Default is P2TR (BIP-86). BIP-84 remains only for exchanges that still do
// not support P2TR. Not a Cargo feature.
// fn xprv_segwit_desc(xprv: &Xpriv, path: &str, change: u32) -> Result<String, BitcoinKeysError> {
//     let xprv = get_segwit_descriptor(xprv, path, change)?;
//     let desc = format!("wpkh({xprv})");
//     Ok(desc)
// }
//
// fn xpub_segwit_desc(xprv: &Xpriv, path: &str, change: u32) -> Result<String, BitcoinKeysError> {
//     let secp = Secp256k1::new();
//     let xprv = get_segwit_descriptor(xprv, path, change)?;
//     let xpub = xprv.to_public(&secp)?;
//     let desc = format!("wpkh({xpub})");
//     Ok(desc)
// }

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

pub async fn new_mnemonic(
    seed_password: &SecretString,
) -> Result<DecryptedWalletData, BitcoinKeysError> {
    let mut entropy = [0u8; 32];
    getrandom_02::getrandom(&mut entropy)?;
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)?;
    entropy.zeroize();

    get_mnemonic(mnemonic, seed_password).await
}

pub async fn save_mnemonic(
    mnemonic_phrase: &SecretString,
    seed_password: &SecretString,
) -> Result<DecryptedWalletData, BitcoinKeysError> {
    let mnemonic = Mnemonic::from_str(&mnemonic_phrase.0)?;

    get_mnemonic(mnemonic, seed_password).await
}

pub async fn get_mnemonic(
    mnemonic_phrase: Mnemonic,
    seed_password: &SecretString,
) -> Result<DecryptedWalletData, BitcoinKeysError> {
    let seed = mnemonic_phrase.to_seed_normalized(&seed_password.0);

    let network = NETWORK.read().await;
    let xprv = Xpriv::new_master(*network, &seed)?;
    let xprvkh = sha256::Hash::hash(&xprv.private_key.secret_bytes()).to_string();

    let secp = Secp256k1::new();
    let xpub = Xpub::from_priv(&secp, &xprv);
    let xpubkh = xpub.to_pub().pubkey_hash().to_string();

    let btc_path = get_btc_path().await;
    let rgb_path = get_rgb_path().await;

    let btc_descriptor_xprv = xprv_desc(&xprv, &btc_path, 0)?;
    let btc_descriptor_xpub = xpub_desc(&xprv, &btc_path, 0)?;

    let btc_change_descriptor_xprv = xprv_desc(&xprv, &btc_path, 1)?;
    let btc_change_descriptor_xpub = xpub_desc(&xprv, &btc_path, 1)?;

    let rgb_descriptor_xprv = xprv_desc(&xprv, &rgb_path, 10)?;
    let rgb_descriptor_xpub = xpub_desc(&xprv, &rgb_path, 10)?;

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

    let private = PrivateWalletData {
        xprvkh,
        btc_descriptor_xprv,
        btc_change_descriptor_xprv,
        rgb_descriptor_xprv,
        nostr_prv,
        nostr_nsec,
    };

    let public = PublicWalletData {
        xpubkh,
        btc_descriptor_xpub,
        btc_change_descriptor_xpub,
        rgb_descriptor_xpub,
        nostr_pub,
        nostr_npub,
    };

    Ok(DecryptedWalletData {
        mnemonic: mnemonic_phrase.to_string(),
        private,
        public,
    })
}
