use crate::bitcoin::publish_psbt_file_bp;
use crate::bp::models::{Balance, BlockTime, BtcAccount, TransactionData, TransactionDetails};
use crate::bp::models::{WalletData, WalletTransaction};
use crate::bp::signer::MainSigner;
use crate::bp::transfer::{BtcTransferError, DrainUtxoInputs, build_psbt, drain_psbt};
use crate::constants::{bp_network_from_str, get_diba_fee, get_network};
use crate::debug;
use crate::rgb::models::{AssetType, RgbAccount};
use crate::rgb::prefetch;
use crate::structs::{FundVaultDetails, PublishPsbtRequest, SecretString};
use crate::wallet::data::{
    AccountCrossStore, BPWallet, CrossStoreAsync, CrossStoreError, SyncProvider,
};
use crate::wallet::interop::XprivBpKey;
use crate::wallet::server::ServerErrors;
#[cfg(target_arch = "wasm32")]
use crate::wallet::watcher::WatcherError;
use crate::wallet::watcher::{watcher_inspect, watcher_prefetch};
use bitcoin::constants::WITNESS_SCALE_FACTOR;
use bpstd::psbt::PsbtConstructor;
use bpstd::{Address, PsbtVer, Sats, XprivAccount};
use bpwallet::{OpType, TxStatus};
use std::collections::BTreeMap;
use std::str::FromStr;

#[cfg(target_arch = "wasm32")]
use crate::bp::server::{backup_btc_account, restore_btc_account};

pub mod constants;
pub mod data;
pub mod models;
pub mod recover;
mod server;
pub mod signer;
mod transfer;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum BtcAccountError {
    /// some request data is invalid. {0:?}
    Validation(BTreeMap<String, String>),
    /// I/O or connectivity error. {0}
    IO(CrossStoreError),
    /// retrieve server data error. {0}
    Server(ServerErrors),
    /// transfer error. {0}
    Transfer(BtcTransferError),
    /// account sync error. {0}
    Wallet(String),
    /// rgb store fail. {0}
    Store(String),
    /// block esplorer sync error. {0}
    Explorer(String),
    /// parse network error
    WrongNetwork,
    /// psbt parse error. {0}
    PsbtError(#[from] bpstd::PsbtParseError),
    /// prefetch error. {0}
    PrefetchError(#[from] prefetch::PrefetchError),
    /// not funds
    NoFunds,
}

pub async fn get_btc_wallet(sk: &str) -> Result<WalletData, BtcAccountError> {
    watcher_prefetch(sk, "bitcoin")
        .await
        .map_err(|err| BtcAccountError::Explorer(err.to_string()))?;

    bp_network_from_str(&get_network().await)
        .map_err(|err| BtcAccountError::Explorer(err.to_string()))?;

    let mut btc_account_provider =
        AccountCrossStore::btc(sk, &get_network().await).map_err(BtcAccountError::IO)?;

    btc_account_provider
        .fetch()
        .await
        .map_err(BtcAccountError::IO)?;

    let mut btc_account: BtcAccount = BtcAccount::get(btc_account_provider.clone())
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;

    let btc_keychain = AssetType::Bitcoin.to_keychain();
    let address = btc_account.wallet.next_address(btc_keychain, true);

    let mut btc_wallet = BPWallet::get(btc_account_provider.clone())
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;

    // Sync wallet before reading balance/utxos/history
    btc_wallet
        .sync(&mut btc_account_provider)
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;
    btc_account_provider
        .persist()
        .await
        .map_err(BtcAccountError::IO)?;
    let mut balance = Balance::default();
    balance.confirmed = btc_wallet.balance().sats();
    debug!(format!("balance: {balance:?}"));

    let unspent_utxos = btc_wallet.utxos().map(|x| x.outpoint.to_string()).collect();

    let transactions: Vec<_> = btc_wallet
        .history()
        .map(|tx| {
            // `bpwallet::TxRow` stores its confirmation status inside the `height` field as a
            // `TxStatus`. We derive `confirmed` and (when possible) `confirmation_time` from it.
            debug!(format!("transact: {tx:?}"));
            let (confirmed, confirmation_time) = match &tx.height {
                TxStatus::Mined(height) => (
                    true,
                    Some(BlockTime {
                        height: height.get(),
                        timestamp: 0, // <- real timestamp from bp-wallet
                    }),
                ),
                _ => (false, None),
            };

            // Compute virtual size in vbytes (ceil(weight / 4)) and fee rate in sat/vB
            let vsize = (tx.weight + 3) / (WITNESS_SCALE_FACTOR as u32);
            let fee_sats = tx.fee.sats();
            let fee_rate = if vsize > 0 {
                fee_sats as f32 / vsize as f32
            } else {
                0.0
            };

            // Use signed net amount to derive received/sent split
            let raw = tx.amount.sats() as i64;
            let signed = match tx.operation {
                OpType::Credit => raw,
                _ => -raw,
            };
            let (received, sent) = if signed >= 0 {
                (signed as u64, 0u64)
            } else {
                (0u64, (-signed) as u64)
            };

            let _ = tx.operation;
            WalletTransaction {
                txid: tx.txid,
                received,
                sent,
                fee: Some(fee_sats),
                confirmed,
                confirmation_time,
                vsize,
                fee_rate,
            }
        })
        .collect();

    // wallet already synced above

    Ok(WalletData {
        balance,
        transactions,
        utxos: unspent_utxos,
        address: address.to_string(),
    })
}

#[derive(Clone, Default)]
/// Options used in fund operation
pub struct FundOptions {
    /// Feature flag to enable publish PSBT
    pub broadcast: bool,
    /// Private key(s) for PSBT signing
    pub private_keys: Vec<SecretString>,
    /// Fixed value for (re-)fund rgb wallet
    pub fund_value: Option<Sats>,
    /// Feature flag to enable one time fee
    pub onetime_fee: bool,
}

impl FundOptions {
    pub fn with(broadcast: bool, private_keys: Vec<SecretString>) -> Self {
        Self {
            broadcast,
            private_keys,
            ..Default::default()
        }
    }
}

pub async fn fund_vault(
    sk: &str,
    fee_rate: f32,
    options: FundOptions,
) -> Result<FundVaultDetails, BtcAccountError> {
    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| BtcAccountError::Explorer(err.to_string()))?;

    watcher_prefetch(sk, "bitcoin")
        .await
        .map_err(|err| BtcAccountError::Explorer(err.to_string()))?;
    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| BtcAccountError::Explorer(err.to_string()))?;

    let mut btc_account_provider =
        AccountCrossStore::btc(sk, &get_network().await).map_err(BtcAccountError::IO)?;

    btc_account_provider
        .fetch()
        .await
        .map_err(BtcAccountError::IO)?;

    let mut btc_account: BtcAccount = BtcAccount::get(btc_account_provider.clone())
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(BtcAccountError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(BtcAccountError::IO)?;

    let mut rgb_account: RgbAccount = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;

    let rgb_keychain = AssetType::RGB20.to_keychain();
    let address = rgb_account.wallet.next_address(rgb_keychain, false);

    let FundOptions {
        private_keys: secrets,
        broadcast,
        fund_value,
        onetime_fee,
        ..
    } = options.clone();

    // First Time Fund
    let rgb_ops = rgb_account.wallet.cache_l1();
    let onetime_fee = if onetime_fee && rgb_ops.tx.is_empty() {
        let diba_addr = get_diba_fee().await;
        let diba_addr = Address::from_str(&diba_addr).expect("invalid diba address");

        let diba_fee = Sats(3000);
        Some((diba_addr, diba_fee))
    } else {
        None
    };

    btc_account.wallet.utxos().for_each(|x| {
        debug!(format!(
            "btc_utxo: {}@{} ({})",
            x.outpoint,
            x.value,
            x.status.is_mined()
        ));
    });
    debug!(format!("fund_rgb_vault: {} ({})", address, network));

    let (mut psbt, _) = build_psbt(&mut btc_account, fee_rate, address, fund_value, onetime_fee)
        .await
        .map_err(BtcAccountError::Transfer)?;

    let signers: Vec<_> = secrets
        .iter()
        .map(|x| {
            let interop_key =
                XprivBpKey::new(&x.to_string(), network).expect("invalid parse interop key");
            MainSigner::new(
                XprivAccount::new(*interop_key.xpriv(), interop_key.origin().clone())
                    .expect("invalid parse xpriv account"),
            )
        })
        .collect();

    let mut signs = 0;
    for signer in signers.iter() {
        signs += psbt.sign(signer).expect("Error signing btc account");
    }
    debug!(format!("fund sign: {signs}"));
    psbt.finalize(btc_account.wallet.descriptor());

    if broadcast {
        let req = PublishPsbtRequest {
            psbt: psbt.to_base64_ver(PsbtVer::V0),
        };
        let _ = publish_psbt_file_bp(req).await.expect("publish tx fail");
    }

    let rgb_output = psbt
        .outputs()
        .find(|x| x.script == address.script_pubkey())
        .map(|x| x.index());

    btc_account
        .sync(&mut btc_account_provider)
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;
    btc_account_provider
        .persist()
        .await
        .map_err(BtcAccountError::IO)?;

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;
    rgb_account_provider
        .persist()
        .await
        .map_err(BtcAccountError::IO)?;

    Ok(FundVaultDetails {
        is_funded: true,
        rgb_output: Some(format!("{}:{}", psbt.txid(), rgb_output.unwrap())),
        fund_txid: Some(psbt.txid().to_string()),
        fund_fee: Some(psbt.fee().unwrap_or_default().sats() as u32),
    })
}

#[derive(Clone, Default)]
/// Options used in fund operation
pub struct DrainOptions {
    /// Feature flag to enable publish PSBT
    pub broadcast: bool,
    /// Private key(s) for PSBT signing
    pub private_keys: Vec<SecretString>,
    /// Collection of UTXOs
    pub utxos: DrainUtxoInputs,
}

impl DrainOptions {
    pub fn with(broadcast: bool, private_keys: Vec<SecretString>) -> Self {
        Self {
            broadcast,
            private_keys,
            ..Default::default()
        }
    }
}

pub async fn drain_vault(
    sk: &str,
    recover_address: &str,
    options: DrainOptions,
) -> Result<FundVaultDetails, BtcAccountError> {
    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| BtcAccountError::Explorer(err.to_string()))?;

    watcher_inspect(sk, "bitcoin")
        .await
        .map_err(|err| BtcAccountError::Explorer(err.to_string()))?;

    let mut btc_account_provider =
        AccountCrossStore::btc_recover(sk, &get_network().await).map_err(BtcAccountError::IO)?;

    btc_account_provider
        .fetch()
        .await
        .map_err(BtcAccountError::IO)?;

    let mut btc_account: BtcAccount = BtcAccount::get(btc_account_provider.clone())
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;

    let DrainOptions {
        private_keys: secrets,
        broadcast,
        utxos,
        ..
    } = options.clone();

    let mut recover_sats = Sats::ZERO;
    utxos
        .iter()
        .flat_map(|(_, data)| data.iter().map(|(x, _)| x))
        .for_each(|x| {
            debug!(format!(
                "btc_utxo: {}@{} ({})",
                x.outpoint,
                x.value,
                x.status.is_mined()
            ));
            if matches!(x.status, TxStatus::Unknown | TxStatus::Mined(_)) {
                recover_sats += x.value;
            }
        });

    if recover_sats.is_zero() {
        return Err(BtcAccountError::NoFunds);
    }

    let recover_address = Address::from_str(recover_address).expect("invalid address");
    let (mut psbt, _) = drain_psbt(&mut btc_account, 1.0, recover_address, recover_sats, utxos)
        .await
        .map_err(BtcAccountError::Transfer)?;

    let signers: Vec<_> = secrets
        .iter()
        .map(|x| {
            let interop_key =
                XprivBpKey::new(&x.to_string(), network).expect("invalid parse interop key");
            MainSigner::new(
                XprivAccount::new(*interop_key.xpriv(), interop_key.origin().clone())
                    .expect("invalid parse xpriv account"),
            )
        })
        .collect();

    let mut signs = 0;
    for signer in signers.iter() {
        signs += psbt.sign(signer).expect("Error signing btc account");
    }
    debug!(format!("fund sign: {signs}"));
    psbt.finalize(btc_account.wallet.descriptor());

    if broadcast {
        let req = PublishPsbtRequest {
            psbt: psbt.to_base64_ver(PsbtVer::V0),
        };
        let _ = publish_psbt_file_bp(req).await.expect("publish tx fail");
    }

    btc_account
        .sync(&mut btc_account_provider)
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;
    btc_account_provider
        .persist()
        .await
        .map_err(BtcAccountError::IO)?;

    Ok(FundVaultDetails {
        is_funded: true,
        rgb_output: None,
        fund_txid: Some(psbt.txid().to_string()),
        fund_fee: Some(psbt.fee().unwrap_or_default().sats() as u32),
    })
}

pub async fn transfer_btc(
    sk: &str,
    address: &str,
    sats: u64,
    fee_rate: f32,
    secrets: Vec<SecretString>,
) -> Result<TransactionData, BtcAccountError> {
    watcher_prefetch(sk, "bitcoin")
        .await
        .map_err(|err| BtcAccountError::Explorer(err.to_string()))?;

    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| BtcAccountError::Explorer(err.to_string()))?;

    let mut btc_account_provider =
        AccountCrossStore::btc(sk, &get_network().await).map_err(BtcAccountError::IO)?;

    btc_account_provider
        .fetch()
        .await
        .map_err(BtcAccountError::IO)?;

    let mut btc_account: BtcAccount = BtcAccount::get(btc_account_provider.clone())
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;

    let address = Address::from_str(address).expect("Invalid address string");
    let sats = Sats::from_sats(sats);

    let (mut psbt, change) = build_psbt(&mut btc_account, fee_rate, address, Some(sats), None)
        .await
        .map_err(BtcAccountError::Transfer)?;

    let signers: Vec<_> = secrets
        .iter()
        .map(|x| {
            let interop_key =
                XprivBpKey::new(&x.to_string(), network).expect("invalid parse interop key");
            MainSigner::new(
                XprivAccount::new(*interop_key.xpriv(), interop_key.origin().clone())
                    .expect("invalid parse xpriv account"),
            )
        })
        .collect();

    for signer in signers.iter() {
        psbt.sign(signer).expect("Error signing btc account");
    }

    psbt.finalize(btc_account.wallet.descriptor());

    let req = PublishPsbtRequest {
        psbt: psbt.to_base64_ver(PsbtVer::V0),
    };
    let _ = publish_psbt_file_bp(req).await.expect("publish tx fail");

    btc_account
        .sync(&mut btc_account_provider)
        .map_err(|err| BtcAccountError::Wallet(err.to_string()))?;
    btc_account_provider
        .persist()
        .await
        .map_err(BtcAccountError::IO)?;

    let final_tx = psbt.extract().expect("Error extracting psbt");
    let details = TransactionDetails {
        transaction: Some(final_tx.clone()),
        txid: final_tx.txid(),
        received: change,
        sent: sats.sats(),
        fee: psbt.fee().map(|x| x.sats()),
        confirmation_time: None,
    };

    Ok(TransactionData {
        details,
        vsize: 0,
        fee_rate,
    })
}

#[cfg(target_arch = "wasm32")]
pub async fn btc_backup(sk: &str) -> Result<bool, WatcherError> {
    watcher_prefetch(sk, "bitcoin").await?;

    let network =
        bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let mut rgb_account_provider =
        AccountCrossStore::btc(sk, &get_network().await).map_err(WatcherError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(WatcherError::IO)?;

    backup_btc_account(sk, &mut rgb_account_provider)
        .await
        .map_err(WatcherError::Server)?;
    Ok(true)
}

#[cfg(target_arch = "wasm32")]
pub async fn btc_restore(sk: &str) -> Result<bool, WatcherError> {
    let network =
        bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let mut rgb_account_provider =
        AccountCrossStore::btc(sk, &get_network().await).map_err(WatcherError::IO)?;

    let mut rgb_account = restore_btc_account(sk, &mut rgb_account_provider)
        .await
        .map_err(WatcherError::Server)?;

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| WatcherError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| WatcherError::Store(err.to_string()))?;

    Ok(true)
}
