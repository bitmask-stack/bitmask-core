//! Wallet descriptor encrypt/decrypt.
//!
//! Wire format matches serde-encrypt 0.7 `SerdeEncryptSharedKey` +
//! `BincodeSerializer`: bincode of the struct, then XChaCha20-Poly1305,
//! then `nonce (24 bytes) || ciphertext`. Shared key is the 32-byte Argon2id
//! hash. Public-key X25519 is not used. This path does not depend on
//! crypto_box / curve25519-dalek.

use chacha20poly1305::{
    Key, XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};

use rand::RngCore;
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
#[cfg(all(test, target_arch = "wasm32"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

const NONCE_SIZE: usize = 24;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedMessage {
    encrypted: Vec<u8>,
    nonce: [u8; NONCE_SIZE],
}

#[derive(Error, Debug)]
pub enum WalletEncryptError {
    #[error("bincode: {0}")]
    Bincode(#[from] bincode::Error),
    #[error("XChaCha20-Poly1305 encrypt/decrypt failed")]
    Aead,
    #[error("encrypted payload is shorter than the 24-byte nonce")]
    MissingNonce,
}

impl EncryptedMessage {
    pub fn serialize(mut self) -> Vec<u8> {
        let mut serialized: Vec<u8> = self.nonce.to_vec();
        serialized.append(&mut self.encrypted);
        serialized
    }

    pub fn deserialize(mut serialized: Vec<u8>) -> Result<Self, WalletEncryptError> {
        if serialized.len() < NONCE_SIZE {
            return Err(WalletEncryptError::MissingNonce);
        }
        let encrypted = serialized.split_off(NONCE_SIZE);
        let nonce: [u8; NONCE_SIZE] = serialized
            .try_into()
            .map_err(|_| WalletEncryptError::MissingNonce)?;
        Ok(Self { encrypted, nonce })
    }
}

pub fn encrypt_owned<T: Serialize>(
    value: &T,
    shared_key: &[u8; 32],
) -> Result<EncryptedMessage, WalletEncryptError> {
    let serialized = bincode::serialize(value)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(shared_key));
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let encrypted = cipher
        .encrypt(nonce, serialized.as_ref())
        .map_err(|_| WalletEncryptError::Aead)?;
    Ok(EncryptedMessage {
        encrypted,
        nonce: nonce_bytes,
    })
}

pub fn decrypt_owned<T: DeserializeOwned>(
    encrypted_message: &EncryptedMessage,
    shared_key: &[u8; 32],
) -> Result<T, WalletEncryptError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(shared_key));
    let nonce = XNonce::from_slice(&encrypted_message.nonce);
    let plain = cipher
        .decrypt(nonce, encrypted_message.encrypted.as_ref())
        .map_err(|_| WalletEncryptError::Aead)?;
    Ok(bincode::deserialize(&plain)?)
}

#[test]
fn encrypted_message_is_nonce_then_ciphertext() {
    let msg = EncryptedMessage {
        encrypted: b"*ENCRYPTED*".to_vec(),
        nonce: [42u8; 24],
    };
    let bin = msg.clone().serialize();
    assert_eq!(&bin[..24], &[42u8; 24]);
    assert_eq!(&bin[24..], b"*ENCRYPTED*");
    assert_eq!(EncryptedMessage::deserialize(bin).unwrap(), msg);
}

#[test]
fn encrypted_message_rejects_short_nonce() {
    let err = EncryptedMessage::deserialize(vec![1, 2, 3]).unwrap_err();
    assert!(matches!(err, WalletEncryptError::MissingNonce));
}

#[test]
fn bincode_xchacha20_roundtrip_v04_fields() {
    use crate::structs::EncryptedWalletDataV04;

    let key = [7u8; 32];
    let original = EncryptedWalletDataV04 {
        btc_descriptor_xprv: "xprv-a".to_owned(),
        btc_descriptor_xpub: "xpub-a".to_owned(),
        btc_change_descriptor_xprv: "xprv-b".to_owned(),
        btc_change_descriptor_xpub: "xpub-b".to_owned(),
        rgb_assets_descriptor_xprv: "xprv-c".to_owned(),
        rgb_assets_descriptor_xpub: "xpub-c".to_owned(),
        rgb_udas_descriptor_xprv: "xprv-d".to_owned(),
        rgb_udas_descriptor_xpub: "xpub-d".to_owned(),
        xprvkh: "kh-prv".to_owned(),
        xpubkh: "kh-pub".to_owned(),
        mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_owned(),
    };
    let encrypted = encrypt_owned(&original, &key).unwrap();
    let decrypted: EncryptedWalletDataV04 = decrypt_owned(&encrypted, &key).unwrap();
    assert_eq!(decrypted.mnemonic, original.mnemonic);
    assert_eq!(decrypted.btc_descriptor_xpub, original.btc_descriptor_xpub);
}

/// Known-answer decrypt of a frozen blob. Ciphertext is IETF
/// XChaCha20-Poly1305 with empty AAD from libsodium
/// `crypto_aead_xchacha20poly1305_ietf` (same construction as
/// serde-encrypt-core 0.7 SharedKey). Plaintext is bincode 1.3 of
/// `EncryptedWalletDataV04` fields `"a"` through `"k"`. Key is 32
/// times `0x07`. Nonce is 24 times `0x11`. Fails if ChaCha key wrap,
/// nonce size, empty AAD, or bincode options drift. Do not produce
/// this blob from `encrypt_owned`.
#[test]
fn decrypts_fixed_serde_encrypt_shared_key_blob() {
    use crate::structs::EncryptedWalletDataV04;

    const KEY: [u8; 32] = [0x07; 32];
    const BLOB_HEX: &str = "4449424100111111111111111111111111111111111111111111111111e3c0ed5308ac14df29e066ce7e6da1ffbff4c13336401d85c994b173180b31cc20c682260fe5d93eec0060ea1788d81cacadd27349108bfa780a26c7fa5a989a6e875d764b428c554f95a7563c6ffa58bf348f3cbf014846f8dcc2749edddb6dba290afcf5a748c1eab17047a1fe3ea65178d8";

    let blob = hex::decode(BLOB_HEX).expect("known-answer hex");
    assert_eq!(
        blob.len(),
        5 + 24 + 99 + 16,
        "DIBA + version + nonce + ct + tag"
    );
    assert_eq!(&blob[..4], b"DIBA", "DIBA magic");
    assert_eq!(blob[4], 0, "descriptor version byte 0");
    assert_eq!(&blob[5..29], &[0x11u8; 24], "24-byte nonce");

    let msg = EncryptedMessage::deserialize(blob[5..].to_vec()).unwrap();
    let decrypted: EncryptedWalletDataV04 = decrypt_owned(&msg, &KEY).unwrap();
    assert_eq!(decrypted.btc_descriptor_xprv, "a");
    assert_eq!(decrypted.btc_descriptor_xpub, "b");
    assert_eq!(decrypted.btc_change_descriptor_xprv, "c");
    assert_eq!(decrypted.btc_change_descriptor_xpub, "d");
    assert_eq!(decrypted.rgb_assets_descriptor_xprv, "e");
    assert_eq!(decrypted.rgb_assets_descriptor_xpub, "f");
    assert_eq!(decrypted.rgb_udas_descriptor_xprv, "g");
    assert_eq!(decrypted.rgb_udas_descriptor_xpub, "h");
    assert_eq!(decrypted.xprvkh, "i");
    assert_eq!(decrypted.xpubkh, "j");
    assert_eq!(decrypted.mnemonic, "k");
}
