use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{constants::LNDHUB_ENDPOINT, util::post_json_auth};

#[cfg(all(test, target_arch = "wasm32"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Nostr pubkey
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Nostr {
    pub pubkey: String,
}

/// Status response
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Response {
    pub status: String,
}

fn validate_pubkey(pubkey: &str) -> Result<String> {
    let pubkey = hex::decode(pubkey)?;

    if pubkey.len() == 32 {
        Ok(hex::encode(pubkey))
    } else if pubkey.len() == 33 && (pubkey[0] == 2 || pubkey[0] == 3) {
        match pubkey.get(1..33) {
            Some(key) => Ok(hex::encode(key)),
            None => Err(anyhow!("Incorrect number of bytes in hex key")),
        }
    } else {
        Err(anyhow!("Hex key is of wrong length or format"))
    }
}

#[test]
fn test_validate_pubkey() -> Result<()> {
    let result =
        validate_pubkey("03b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9")?;

    assert_eq!(
        "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9", result,
        "strips leading parity byte on 33 byte x-only pubkey"
    );

    Ok(())
}

#[test]
fn test_validate_32bytes_pubkey() -> Result<()> {
    let result =
        validate_pubkey("0000066e0359c33a0bed474853a610f744404f265140ecf5171b38483aaea2bb")?;

    assert_eq!(
        "0000066e0359c33a0bed474853a610f744404f265140ecf5171b38483aaea2bb", result,
        "32 bytes pubkey"
    );

    Ok(())
}

fn parse_npub(pubkey: &str) -> Result<String> {
    if pubkey.starts_with("npub") {
        let key = nostr::PublicKey::parse(pubkey)?;
        Ok(key.to_hex())
    } else {
        Ok(pubkey.to_owned())
    }
}

#[test]
fn test_parse_npub() -> Result<()> {
    let result = parse_npub("npub1qqqqqqqx2tj99mng5qgc07cgezv5jm95dj636x4qsq7svwkwmwnse3rfkq")?;

    assert_eq!(
        result,
        "000000000652e452ee68a01187fb08c899496cb46cb51d1aa0803d063acedba7"
    );

    Ok(())
}

/// Parse nsec/npub with nostr 0.44 (`Keys::parse`, `PublicKey::parse`, `xonly`).
/// NIP-06 vectors match `nip06`.
#[test]
fn parse_nsec_npub_and_xonly_pubkey() -> Result<()> {
    use nostr::ToBech32;

    const NSEC: &str = "nsec1t9s9xyu55mezxwkf98uws2m8h6smjvehgngme0346rwm456g3kfs8pt0qw";
    const NPUB: &str = "npub1v5zwxyd3dtmrvgnamxxlfxj92t52hwkg09dmwjhmkjujkq8kdzms77547c";

    let keys = nostr::Keys::parse(NSEC)?;
    assert_eq!(
        keys.public_key()
            .to_bech32()
            .expect("npub bech32 is infallible"),
        NPUB,
        "nsec parses to the matching npub"
    );

    let pk = nostr::PublicKey::parse(NPUB)?;
    let xonly = pk.xonly()?;
    assert_eq!(xonly.serialize().len(), 32, "x-only public key is 32 bytes");
    assert_eq!(pk.to_hex().len(), 64, "x-only pubkey hex is 64 chars");
    assert_eq!(
        hex::encode(xonly.serialize()),
        pk.to_hex(),
        "x-only serialize matches PublicKey hex"
    );
    assert_eq!(keys.public_key().to_hex(), pk.to_hex());

    Ok(())
}

/// Add a new nostr pubkey to a user
pub async fn new_nostr_pubkey(pubkey: &str, token: &str) -> Result<Response> {
    let pubkey = parse_npub(pubkey)?;
    let pubkey = validate_pubkey(&pubkey)?;

    let endpoint = LNDHUB_ENDPOINT.read().await;
    let pubkey = Nostr {
        pubkey: pubkey.to_string(),
    };
    let url = format!("{endpoint}/nostr_pubkey");
    let response = post_json_auth(&url, &Some(pubkey), Some(token)).await?;

    let res: Response = serde_json::from_str(&response)?;

    Ok(res)
}

/// Update the user nostr pubkey
pub async fn update_nostr_pubkey(pubkey: &str, token: &str) -> Result<Response> {
    let pubkey = parse_npub(pubkey)?;
    let pubkey = validate_pubkey(&pubkey)?;

    let endpoint = LNDHUB_ENDPOINT.read().await;
    let pubkey = Nostr {
        pubkey: pubkey.to_string(),
    };
    let url = format!("{endpoint}/update_nostr_pubkey");
    let response = post_json_auth(&url, &Some(pubkey), Some(token)).await?;

    let res: Response = serde_json::from_str(&response)?;

    Ok(res)
}
