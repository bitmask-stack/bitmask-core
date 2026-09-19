#![cfg(not(target_arch = "wasm32"))]

use bitmask_core::bitcoin::new_mnemonic;
use bitmask_core::rgb::{RgbAccountError, get_contract, import_contract};
use bitmask_core::structs::SecretString;

/// Empty `import_contract` returns before explorer. Nix nextest runs this.
#[tokio::test]
async fn import_contract_empty_is_validation() {
    let keys = new_mnemonic(&SecretString("".into())).await.unwrap();
    let err = import_contract(&keys.private.nostr_prv, String::new())
        .await
        .unwrap_err();
    match err {
        RgbAccountError::Validation(map) => {
            assert_eq!(map.get("contract").unwrap(), "parameter is required")
        }
        other => panic!("{other}"),
    }
}

/// Garbage armored contract is parse Validation, not explorer.
#[tokio::test]
async fn import_contract_garbage_is_parse() {
    let keys = new_mnemonic(&SecretString("".into())).await.unwrap();
    let err = import_contract(&keys.private.nostr_prv, "not-a-contract".into())
        .await
        .unwrap_err();
    match err {
        RgbAccountError::Validation(map) => {
            assert_eq!(map.get("contract").unwrap(), "parse error")
        }
        other => panic!("{other}"),
    }
}

/// Empty `get_contract` returns before explorer.
#[tokio::test]
async fn get_contract_empty_is_validation() {
    let keys = new_mnemonic(&SecretString("".into())).await.unwrap();
    let err = get_contract(&keys.private.nostr_prv, String::new())
        .await
        .unwrap_err();
    match err {
        RgbAccountError::Validation(map) => {
            assert_eq!(map.get("contract").unwrap(), "parameter is required")
        }
        other => panic!("{other}"),
    }
}

/// Unparseable contract id is `"contract_id"` parse Validation.
#[tokio::test]
async fn get_contract_garbage_is_parse() {
    let keys = new_mnemonic(&SecretString("".into())).await.unwrap();
    let err = get_contract(&keys.private.nostr_prv, "zzzz".into())
        .await
        .unwrap_err();
    match err {
        RgbAccountError::Validation(map) => {
            assert_eq!(map.get("contract_id").unwrap(), "parse error")
        }
        other => panic!("{other}"),
    }
}
