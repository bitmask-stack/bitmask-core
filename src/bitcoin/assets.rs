use anyhow::Result;
use bdk_wallet::bitcoin::{Amount, FeeRate, psbt::Psbt};
use bdk_wallet::{LocalOutput, SignOptions, Wallet};

use crate::bitcoin::fee_rate_from_sat_vb;
use crate::debug;

pub fn dust_tx(
    btc_wallet: &mut Wallet,
    fee_rate: f32,
    first_utxo: Option<&LocalOutput>,
) -> Result<Psbt> {
    let dust_amt = if fee_rate < 3.0 {
        546
    } else {
        (182.0 * fee_rate).ceil() as u64
    };

    let pubkey = match first_utxo {
        Some(utxo) => utxo.txout.script_pubkey.clone(),
        None => anyhow::bail!("dust_tx needs a UTXO"),
    };

    let mut tx_builder = btc_wallet.build_tx();
    tx_builder.add_recipient(pubkey, Amount::from_sat(dust_amt));
    tx_builder.fee_rate(
        FeeRate::from_sat_per_vb(fee_rate.ceil() as u64)
            .unwrap_or_else(|| fee_rate_from_sat_vb(1.0)),
    );
    let mut dust_psbt = tx_builder.finish()?;

    // dust_tx only has Wallet. Wallet::sign is deprecated in bdk_wallet 3.2.0.
    #[allow(deprecated)]
    let finalized = btc_wallet.sign(&mut dust_psbt, SignOptions::default())?;
    debug!(format!("PSBT signed. Finalized: {finalized}"));
    btc_wallet.finalize_psbt(&mut dust_psbt, SignOptions::default())?;
    debug!(format!("PSBT finalized. Finalized: {finalized}"));

    Ok(dust_psbt)
}
