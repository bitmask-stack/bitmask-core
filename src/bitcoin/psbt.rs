use crate::{
    bitcoin::{MemoryWallet, get_blockchain},
    debug, info,
    structs::TransactionDetails,
};
use bdk_wallet::SignOptions;
use bitcoin::{consensus::serialize, psbt::Psbt};
use thiserror::Error;

#[allow(clippy::result_large_err)] // PSBT + esplora error graph; do not reshape
#[derive(Error, Debug)]
pub enum BitcoinPsbtError {
    /// Could not broadcast PSBT
    #[error("Could not broadcast PSBT. {0}")]
    CouldNotBroadcastPsbt(String),
    /// Could not finalize when signing PSBT
    #[error("Could not finalize when signing PSBT")]
    CouldNotFinalizePsbt,
    /// BDK signer error
    #[error("BDK sign: {0}")]
    BdkSignError(String),
    /// BDK esplora error
    #[error(transparent)]
    BdkIndexerError(#[from] bdk_esplora::esplora_client::Error),
    /// BDK wallet error
    #[error(transparent)]
    BitcoinWalletError(#[from] crate::bitcoin::BitcoinWalletError),
    /// RGB/BP esplora (bp-esplora crate name `esplora`)
    #[error(transparent)]
    Esplora(#[from] esplora::Error),
    #[error("No fee in PSBT present")]
    NoFeeInPsbt,
    #[error("PSBT extract tx: {0}")]
    ExtractTx(String),
}

fn sign_opts() -> SignOptions {
    SignOptions {
        allow_all_sighashes: true,
        ..Default::default()
    }
}

fn psbt_fee_sats(psbt: &Psbt) -> Result<u64, BitcoinPsbtError> {
    psbt.fee()
        .map(|a| a.to_sat())
        .map_err(|_| BitcoinPsbtError::NoFeeInPsbt)
}

fn extract_tx(psbt: Psbt) -> Result<bitcoin::Transaction, BitcoinPsbtError> {
    psbt.extract_tx()
        .map_err(|e| BitcoinPsbtError::ExtractTx(e.to_string()))
}

fn tx_hex(tx: &bitcoin::Transaction) -> String {
    serialize(tx).iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn details_from_broadcast_tx(
    tx: Option<bitcoin::Transaction>,
    txid: bitcoin::Txid,
    fee_amount: u64,
) -> TransactionDetails {
    let mut sent = 0;
    let mut received = 0;

    if let Some(ref transaction) = tx {
        sent = transaction
            .output
            .iter()
            .fold(0, |sum, output| output.value.to_sat() + sum);
        received = sent.saturating_sub(fee_amount);
    }

    TransactionDetails {
        transaction: tx,
        txid,
        received,
        sent,
        fee: Some(fee_amount),
        confirmation_time: None,
    }
}

pub async fn sign_psbt(wallet: &MemoryWallet, mut psbt: Psbt) -> Result<Psbt, BitcoinPsbtError> {
    debug!("Funding PSBT...");
    wallet
        .lock()
        .await
        .sign(&mut psbt, sign_opts())
        .map_err(|e| BitcoinPsbtError::BdkSignError(e.to_string()))?;
    Ok(psbt)
}

pub async fn multi_sign_psbt(
    wallets: Vec<MemoryWallet>,
    mut psbt: Psbt,
) -> Result<Psbt, BitcoinPsbtError> {
    let total_wallets = wallets.len();
    debug!(format!(
        "Signing PSBT ({total_wallets}/{total_wallets}) ..."
    ));

    let mut sign_count = 0;
    for wallet in wallets {
        let current_wallet = wallet.lock().await;
        let sign = current_wallet
            .sign(&mut psbt, sign_opts())
            .map_err(|e| BitcoinPsbtError::BdkSignError(e.to_string()))?;

        if sign {
            sign_count += 1;
        }

        debug!(format!("PSBT Sign: ({sign_count}/{total_wallets})"));
    }

    Ok(psbt)
}

pub async fn publish_psbt(psbt: Psbt) -> Result<TransactionDetails, BitcoinPsbtError> {
    debug!("Signed PSBT:", base64::encode(&psbt.serialize()));

    let fee_amount = psbt_fee_sats(&psbt)?;
    let tx = extract_tx(psbt)?;
    debug!("tx:", &tx_hex(&tx));
    let blockchain = get_blockchain().await?;
    blockchain
        .broadcast(&tx)
        .await
        .map_err(|op| BitcoinPsbtError::CouldNotBroadcastPsbt(op.to_string()))?;

    let txid = tx.compute_txid();
    let fetched = blockchain.get_tx(&txid).await?;

    Ok(details_from_broadcast_tx(fetched, txid, fee_amount))
}

/// Signs and broadcasts a transaction given a Psbt
pub async fn sign_and_publish_psbt(
    wallet: &MemoryWallet,
    mut psbt: Psbt,
    broadcast: bool,
) -> Result<TransactionDetails, BitcoinPsbtError> {
    info!("Signing PSBT...");
    let finalized = wallet
        .lock()
        .await
        .sign(&mut psbt, SignOptions::default())
        .map_err(|e| BitcoinPsbtError::BdkSignError(e.to_string()))?;
    info!(format!("Finalized: {finalized}"));
    if finalized {
        debug!("Signed PSBT:", base64::encode(&psbt.serialize()));

        let fee_amount = psbt_fee_sats(&psbt)?;

        info!(format!("Fee amount: {fee_amount}"));
        let tx = extract_tx(psbt)?;
        debug!("tx:", &tx_hex(&tx));
        let blockchain = get_blockchain().await?;

        if broadcast {
            blockchain.broadcast(&tx).await?;
        }

        let txid = tx.compute_txid();
        let fetched = blockchain.get_tx(&txid).await?;

        Ok(details_from_broadcast_tx(fetched, txid, fee_amount))
    } else {
        Err(BitcoinPsbtError::CouldNotFinalizePsbt)
    }
}

/// Signs and broadcasts a transaction given a Psbt
pub async fn multi_sign_and_publish_psbt(
    wallets: Vec<MemoryWallet>,
    mut psbt: Psbt,
) -> Result<TransactionDetails, BitcoinPsbtError> {
    let total_wallets = wallets.len();
    debug!(format!(
        "Signing PSBT ({total_wallets}/{total_wallets}) ..."
    ));

    let mut sign_count = 0;
    let mut finalized = false;
    for wallet in wallets {
        finalized = wallet
            .lock()
            .await
            .sign(&mut psbt, sign_opts())
            .map_err(|e| BitcoinPsbtError::BdkSignError(e.to_string()))?;

        sign_count += 1;
        debug!(format!("PSBT Sign: ({sign_count}/{total_wallets})"));
    }
    debug!(format!("Finalized: {finalized}"));
    if finalized {
        debug!("Signed PSBT:", base64::encode(&psbt.serialize()));

        let fee_amount = psbt_fee_sats(&psbt)?;
        let tx = extract_tx(psbt)?;
        debug!("tx:", &tx_hex(&tx));
        let blockchain = get_blockchain().await?;
        blockchain.broadcast(&tx).await?;

        let txid = tx.compute_txid();
        let fetched = blockchain.get_tx(&txid).await?;

        Ok(details_from_broadcast_tx(fetched, txid, fee_amount))
    } else {
        debug!("incomplete tx:", &tx_hex(&extract_tx(psbt)?));
        Err(BitcoinPsbtError::CouldNotFinalizePsbt)
    }
}
