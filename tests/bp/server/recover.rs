#![cfg(not(target_arch = "wasm32"))]

use bitmask_core::bp::recover::recover_funds_from_bad_wallets;
use bitmask_core::constants::switch_network;
use bitmask_core::structs::SecretString;

#[ignore]
// #[tokio::test]
#[allow(dead_code)]
async fn restore_wallet_and_fund() -> anyhow::Result<()> {
    switch_network("bitcoin").await?;

    let address = ""; // Add destination sats here
    let seed = ""; // Add seed here

    let seed = SecretString(seed.to_string());
    let pass = SecretString("".to_string());

    let recover = recover_funds_from_bad_wallets(seed, pass, address).await;
    assert!(recover.is_ok());
    Ok(())
}
