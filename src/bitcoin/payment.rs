use bdk_wallet::bitcoin::{Amount, FeeRate, Sequence, psbt::Psbt};
use bdk_wallet::bitcoin::{TxIn, psbt::Input};

use payjoin::PjUri;
use payjoin::send::v1::SenderBuilder;
use thiserror::Error;

use crate::{
    bitcoin::{
        fee_rate_from_sat_vb,
        psbt::{BitcoinPsbtError, sign_and_publish_psbt, sign_psbt},
        wallet::MemoryWallet,
    },
    debug, info,
    structs::{SatsInvoice, TransactionDetails},
};

#[allow(clippy::result_large_err)] // payjoin + BDK error graph; do not reshape
#[derive(Error, Debug)]
pub enum BitcoinPaymentError {
    /// Payjoin error response
    #[error("Error performing payjoin: {0}")]
    PayjoinError(String),
    /// BitMask Core Bitcoin Psbt error
    #[error(transparent)]
    BitcoinPsbtError(#[from] BitcoinPsbtError),
    /// BDK create-tx error
    #[error("BDK create tx: {0}")]
    BdkCreateTxError(String),
    /// Payjoin sender builder error
    #[error(transparent)]
    PayjoinBuildSenderError(#[from] payjoin::send::BuildSenderError),
    /// Payjoin Send error
    #[error(transparent)]
    PayjoinSendError(#[from] payjoin::send::ResponseError),
    /// Reqwest error
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
}

pub async fn create_transaction(
    invoices: Vec<SatsInvoice>,
    wallet: &MemoryWallet,
    broadcast: bool,
    fee_rate: Option<FeeRate>,
) -> Result<TransactionDetails, BitcoinPaymentError> {
    let psbt = {
        let mut locked_wallet = wallet.lock().await;
        let mut builder = locked_wallet.build_tx();
        for invoice in invoices {
            builder.add_recipient(
                invoice.address.clone().assume_checked().script_pubkey(),
                Amount::from_sat(invoice.amount),
            );
        }

        builder.set_exact_sequence(Sequence::ENABLE_RBF_NO_LOCKTIME);
        builder.fee_rate(fee_rate.unwrap_or_else(|| fee_rate_from_sat_vb(1.0)));
        builder
            .finish()
            .map_err(|e| BitcoinPaymentError::BdkCreateTxError(e.to_string()))?
    };

    debug!("Unsigned PSBT:", base64::encode(&psbt.serialize()));
    let details = sign_and_publish_psbt(wallet, psbt, broadcast).await?;
    info!("PSBT successfully signed");

    Ok(details)
}

pub async fn create_payjoin(
    invoices: Vec<SatsInvoice>,
    wallet: &MemoryWallet,
    fee_rate: Option<FeeRate>,
    pj_uri: PjUri,
) -> Result<TransactionDetails, BitcoinPaymentError> {
    let enacted_fee_rate = fee_rate.unwrap_or_else(|| fee_rate_from_sat_vb(1.0));
    let psbt = {
        let mut locked_wallet = wallet.lock().await;
        let mut builder = locked_wallet.build_tx();
        for invoice in &invoices {
            builder.add_recipient(
                invoice.address.clone().assume_checked().script_pubkey(),
                Amount::from_sat(invoice.amount),
            );
        }
        builder.fee_rate(enacted_fee_rate);
        builder
            .finish()
            .map_err(|e| BitcoinPaymentError::BdkCreateTxError(e.to_string()))?
    };

    debug!("Unsigned Original PSBT:", base64::encode(&psbt.serialize()));
    let original_psbt = sign_psbt(wallet, psbt.clone()).await?;
    info!("Original PSBT successfully signed");

    let additional_fee_index = psbt
        .unsigned_tx
        .output
        .clone()
        .into_iter()
        .enumerate()
        .find(|(_, txo)| {
            invoices.iter().all(|invoice| {
                txo.script_pubkey != invoice.address.clone().assume_checked().script_pubkey()
            })
        })
        .map(|(i, _)| i);

    let sender = match additional_fee_index {
        Some(index) => {
            let amount_available = psbt
                .clone()
                .unsigned_tx
                .output
                .get(index)
                .map(|o| o.value)
                .unwrap_or(Amount::ZERO);
            const P2TR_INPUT_WEIGHT: u64 = 58; // bitmask is taproot only
            let recommended_fee = enacted_fee_rate
                .fee_wu(bitcoin::Weight::from_wu(P2TR_INPUT_WEIGHT))
                .unwrap_or(Amount::ZERO);
            let max_additional_fee = std::cmp::min(recommended_fee, amount_available);

            SenderBuilder::new(original_psbt.clone(), pj_uri).build_with_additional_fee(
                max_additional_fee,
                Some(index),
                enacted_fee_rate,
                true,
            )?
        }
        None => SenderBuilder::new(original_psbt.clone(), pj_uri)
            .build_non_incentivizing(enacted_fee_rate)?,
    };

    let (req, ctx) = sender.create_v1_post_request();
    info!("Built PayJoin request");
    let response = reqwest::Client::new()
        .post(&req.url)
        .header("Content-Type", req.content_type)
        .body(reqwest::Body::from(req.body))
        .send()
        .await?;
    info!("Got PayJoin response");

    let res = response.text().await?;
    info!(format!("Response: {res}"));

    if res.contains("errorCode") {
        return Err(BitcoinPaymentError::PayjoinError(format!("{res:?}")));
    }

    let payjoin_psbt = ctx.process_response(res.as_bytes())?;
    let payjoin_psbt = add_back_original_input(&original_psbt, payjoin_psbt);

    debug!(
        "Proposed PayJoin PSBT:",
        base64::encode(&payjoin_psbt.serialize())
    );
    let tx = sign_and_publish_psbt(wallet, payjoin_psbt, true).await?;

    Ok(tx)
}

/// Unlike Bitcoin Core's walletprocesspsbt RPC, BDK's finalize_psbt only checks
/// if the script in the PSBT input map matches the descriptor and does not
/// check whether it has control of the OutPoint specified in the unsigned_tx's
/// TxIn. So the original_psbt input data needs to be added back into
/// payjoin_psbt without overwriting receiver input.
fn add_back_original_input(original_psbt: &Psbt, payjoin_psbt: Psbt) -> Psbt {
    fn input_pairs(psbt: &Psbt) -> Box<dyn Iterator<Item = (TxIn, Input)> + '_> {
        Box::new(
            psbt.unsigned_tx
                .input
                .iter()
                .cloned()
                .zip(psbt.inputs.iter().cloned()),
        )
    }

    let mut original_inputs = input_pairs(original_psbt).peekable();

    for (proposed_txin, mut proposed_psbtin) in input_pairs(&payjoin_psbt) {
        if let Some((original_txin, original_psbtin)) = original_inputs.peek()
            && proposed_txin.previous_output == original_txin.previous_output
        {
            proposed_psbtin
                .witness_utxo
                .clone_from(&original_psbtin.witness_utxo);
            proposed_psbtin
                .non_witness_utxo
                .clone_from(&original_psbtin.non_witness_utxo);
            original_inputs.next();
        }
    }
    payjoin_psbt
}
