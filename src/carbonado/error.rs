use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug, Display)]
#[display(doc_comments)]
pub enum CarbonadoError {
    /// std io error: {0}
    StdIoError(#[from] std::io::Error),
    /// std io error: {0}
    StdStrUtf8Error(#[from] std::str::Utf8Error),
    /// Error decoding hexadecimal-encoded string: {0}
    HexDecodeError(#[from] hex::FromHexError),
    /// Error decoding base64-encoded string: {0}
    Base64DecodeError(#[from] base64::DecodeError),
    /// Error creating secp256k1 key: {0}
    Secp256k1Error(#[from] bpstd::secp256k1::Error),
    /// General Carbonado error: {0}
    CarbonadoError(#[from] carbonado::error::CarbonadoError),
    /// General Carbonado error: {0}
    SerdeJsonError(#[from] serde_json::Error),
    /// JS Error: {0}
    #[cfg(target_arch = "wasm32")]
    JsError(#[from] gloo_utils::errors::JsError),
    /// Serde WASM Error: {0}
    #[cfg(target_arch = "wasm32")]
    SerdeWasm(#[from] serde_wasm_bindgen::Error),
    /// All endpoints failed error
    AllEndpointsFailed,
    /// Wrong Nostr private key
    WrongNostrPrivateKey,
    /// Wrong Nostr public key
    WrongNostrPublicKey,
    /// Auction bundle id must be one path segment (no slash or `..`)
    InvalidAuctionBundleId,
    /// Debug: {0}
    Debug(String),
    /// Error: {0}
    AnyhowError(#[from] anyhow::Error),
    /// No parent found for {0:?}
    MissingParent(PathBuf),
    /// Result JsValue is not a string
    ResultNotString,
}
