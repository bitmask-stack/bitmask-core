#![cfg(target_arch = "wasm32")]
use crate::bp::constants::BTC_DATA_VERSION;
use crate::bp::data::BtcAccountBlob;
use crate::bp::models::BtcAccount;
use crate::carbonado::{retrieve, store};
use crate::constants::storage_keys::ACCOUNT;
use crate::wallet::data::AccountCrossStore;
use crate::wallet::server::ServerErrors;
use bc::stl::LIB_ID_BP_CONSENSUS as LIB_ID_BTC;
use postcard::{from_bytes, to_allocvec};

pub async fn backup_btc_account(
    sk: &str,
    provider: &mut AccountCrossStore,
) -> Result<(), ServerErrors> {
    let name = ACCOUNT;
    let hashed_name = blake3::hash(format!("{LIB_ID_BTC}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");

    let dump: BtcAccountBlob = provider.btc_backup().await.expect("invalid dump");
    let content = serde_json::to_string(&dump)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    let data = to_allocvec(&content)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    store(sk, main_name, &data, false, Some(BTC_DATA_VERSION.to_vec()))
        .await
        .map_err(|op| ServerErrors::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

pub async fn restore_btc_account(
    sk: &str,
    provider: &mut AccountCrossStore,
) -> Result<BtcAccount, ServerErrors> {
    let name = ACCOUNT;
    let hashed_name = blake3::hash(format!("{LIB_ID_BTC}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _metadata) = retrieve(sk, main_name, vec![])
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        return Err(ServerErrors::NoFile(name.to_string()));
    }
    let data: String =
        from_bytes(&data).map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

    let blob: BtcAccountBlob = serde_json::from_str(&data)
        .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

    let rgb_account = provider.btc_restore(blob).await.expect("invalid backup");
    Ok(rgb_account)
}
