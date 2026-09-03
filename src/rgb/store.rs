use crate::rgb::data::RGBStockCrossStore;
use crate::wallet::data::{AccountCrossStore, CrossStoreAsync, CrossStoreError};
use bpwallet::Network;

pub async fn get_account_and_stock(
    sk: &str,
    _network: Network,
) -> Result<(AccountCrossStore, RGBStockCrossStore), CrossStoreError> {
    let file_network = crate::constants::get_network().await;
    let mut rgb_stock_provider = RGBStockCrossStore::with(sk, &file_network)?;
    let mut rgb_account_provider = AccountCrossStore::rgb(sk, &file_network)?;

    rgb_stock_provider.fetch().await?;
    rgb_account_provider.fetch().await?;

    Ok((rgb_account_provider, rgb_stock_provider))
}

pub async fn save_account_and_stock(
    rgb_account_provider: &mut AccountCrossStore,
    rgb_stock_provider: &mut RGBStockCrossStore,
) -> Result<(), CrossStoreError> {
    rgb_account_provider.persist().await?;
    rgb_stock_provider.persist().await?;

    Ok(())
}
