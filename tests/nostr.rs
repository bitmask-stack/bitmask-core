#![cfg(not(target_arch = "wasm32"))]

use anyhow::{Ok, Result};
use bitmask_core::{
    info,
    lightning::{AuthResponse, CreateWalletResponse, auth, create_wallet},
    nostr::{new_nostr_pubkey, update_nostr_pubkey},
    util::init_logging,
};
use std::{thread, time::Duration};

async fn new_wallet() -> Result<CreateWalletResponse> {
    // we generate a random string to be used as username and password
    let random_number = bip39::rand::random::<u64>();
    let s = hex::encode(random_number.to_le_bytes());
    // We put to sleep the test to avoid hit too fast the API
    thread::sleep(Duration::from_secs(1));
    let res = create_wallet(&s, &s).await?;

    Ok(res)
}

#[tokio::test]
pub async fn new_nostr_pubkey_test() -> Result<()> {
    init_logging("nostr_tests=debug");
    info!("We create user Alice");
    let res = new_wallet().await?;
    let mut alice = String::new();
    if let CreateWalletResponse::Username { username } = res {
        alice = username;
    }
    let alice_response = auth(&alice, &alice).await?;
    thread::sleep(Duration::from_secs(1));

    // Generate new nostr keys
    let keys = nostr::Keys::generate();
    let pubkey = keys.public_key().to_string();

    if let AuthResponse::Result { refresh: _, token } = alice_response {
        let response = new_nostr_pubkey(&pubkey, &token).await?;
        assert_eq!(response.status, "ok".to_string());
    }

    Ok(())
}

#[tokio::test]
pub async fn update_nostr_pubkey_test() -> Result<()> {
    init_logging("nostr_tests=debug");
    info!("We create user Alice");
    let res = new_wallet().await?;
    let mut alice = String::new();
    if let CreateWalletResponse::Username { username } = res {
        alice = username;
    }
    let alice_response = auth(&alice, &alice).await?;
    thread::sleep(Duration::from_secs(1));

    // Generate new nostr keys
    let keys = nostr::Keys::generate();
    let pubkey = keys.public_key().to_string();

    if let AuthResponse::Result { refresh: _, token } = alice_response {
        let response = new_nostr_pubkey(&pubkey, &token).await?;
        assert_eq!(response.status, "ok".to_string());
        // Update the nostr pubkey
        // Generate newer nostr keys
        let keys = nostr::Keys::generate();
        let pubkey = keys.public_key().to_string();
        let response = update_nostr_pubkey(&pubkey, &token).await?;
        assert_eq!(response.status, "ok".to_string());
    }

    Ok(())
}
