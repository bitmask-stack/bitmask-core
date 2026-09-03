#![cfg(not(target_arch = "wasm32"))]

use crate::bp::server::seeder::{WalletSeed, create_new_fund_wallet};
use bitmask_core::bp::get_btc_wallet;
use bitmask_core::structs::PrivateWalletData;

#[tokio::test]
async fn create_new_btc_transfer() -> anyhow::Result<()> {
    let owner_keys = create_new_fund_wallet(WalletSeed::default()).await?;
    let PrivateWalletData {
        nostr_prv: ref sk, ..
    } = owner_keys.private.clone();
    get_btc_wallet(sk).await?;
    Ok(())
}
