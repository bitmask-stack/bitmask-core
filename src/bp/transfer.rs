use crate::bitcoin::fee_rate_from_sat_vb;
use crate::bp::models::BtcAccount;
use crate::rgb::ext::WalletExt;
use bpstd::psbt::{ConstructionError, PsbtConstructor};
use bpstd::{Address, Prevout, Psbt, PsbtVer, Sats, ScriptPubkey, SeqNo};
use bpwallet::XpubDerivable;
use bpwallet::{TxStatus, WalletUtxo};
use rand::prelude::StdRng;
use rand::{Rng, SeedableRng};
use rgb::RgbDescr;

pub(crate) type DrainUtxoInputs = Vec<(RgbDescr<XpubDerivable>, Vec<(WalletUtxo, ScriptPubkey)>)>;

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum BtcTransferError {
    /// diba fee errors. {0}
    Diba(String),
    /// tx errors. {0}
    Tx(#[from] ConstructionError),
    /// wrong psbt input. Invalid {0} prevout
    WrongInput(String),
    /// wrong psbt output. Invalid {0} txout
    WrongOutput(String),
    /// impossible to construct transaction having no outputs.
    NoOutput,
    /// impossible to construct transaction having no output value.
    NoOutputValue,
}

pub async fn build_psbt(
    btc_account: &mut BtcAccount,
    fee_rate: f32,
    transfer_addr: Address,
    transfer_value: Option<Sats>,
    coordinator: Option<(Address, Sats)>,
) -> Result<(Psbt, u64), BtcTransferError> {
    let mut rng = StdRng::from_entropy();
    let mut spend_total = if let Some(sats) = transfer_value {
        sats
    } else {
        Sats(rng.gen_range(589..800))
    };

    let inputs: Vec<_> = btc_account
        .wallet
        .coinselect(spend_total, |x| matches!(x.status, TxStatus::Mined(_)))
        .collect();

    if inputs.is_empty() {
        return Err(BtcTransferError::Tx(ConstructionError::NoInputs));
    };

    // Inputs
    let utxos_inputs: Vec<_> = btc_account
        .wallet
        .utxos()
        .filter(|x| inputs.contains(&x.outpoint))
        .collect();
    let total_balance = utxos_inputs.iter().map(|x| x.value).sum();
    if total_balance < spend_total {
        return Err(BtcTransferError::Tx(
            ConstructionError::OutputExceedsInputs {
                input_value: total_balance,
                output_value: spend_total,
            },
        ));
    }

    // Outputs
    // rgb fun output
    let mut outputs = vec![];
    outputs.push((transfer_addr, spend_total));

    // fee output (one time)
    if let Some((diba_addr, diba_fee)) = coordinator {
        spend_total += diba_fee;
        if total_balance < spend_total {
            return Err(BtcTransferError::Diba(
                ConstructionError::NoFundsForFee {
                    input_value: total_balance,
                    output_value: spend_total,
                    fee: diba_fee,
                }
                .to_string(),
            ));
        }
        outputs.push((diba_addr, diba_fee))
    };

    // change
    let change = total_balance - spend_total;
    let change_address = btc_account.wallet.next_address(1, true);
    outputs.push((change_address, change));

    // PSBT
    let seqno = SeqNo::from_consensus_u32(0);
    let mut psbt = Psbt::create(PsbtVer::V2);

    // Inputs
    for utxo_input in utxos_inputs {
        if let Some((_, spk)) = btc_account.wallet.utxo(utxo_input.outpoint) {
            let WalletUtxo {
                outpoint,
                value,
                terminal,
                ..
            } = utxo_input;
            let prev_out = Prevout::new(outpoint, value);
            psbt.construct_input(
                prev_out,
                btc_account.wallet.descriptor(),
                terminal,
                spk,
                seqno,
            )
            .map_err(|_| BtcTransferError::WrongInput(format!("{outpoint}#{value}")))?;
        }
    }

    // Fee
    let mut fee_rate_psbt = psbt.clone();
    for (address, value) in outputs.clone() {
        fee_rate_psbt
            .construct_output(address.script_pubkey(), value)
            .map_err(|_| BtcTransferError::WrongOutput(format!("{address}#{value}")))?;
    }

    let fee = btc_account
        .wallet
        .fee_estimate(fee_rate_psbt, fee_rate_from_sat_vb(fee_rate))
        .expect("bp transfer");

    let last_index = outputs.iter().position(|(addr, _)| {
        addr.payload.script_pubkey() == change_address.payload.script_pubkey()
    });

    let last_index = if let Some(last_index) = last_index {
        last_index
    } else {
        return Err(BtcTransferError::NoOutput);
    };

    let fee = Sats(fee);
    let change_final =
        change
            .checked_sub(fee)
            .ok_or(BtcTransferError::Tx(ConstructionError::NoFundsForFee {
                input_value: total_balance,
                output_value: change,
                fee,
            }))?;

    outputs.remove(last_index);
    outputs.insert(last_index, (change_address, change_final));

    for (address, value) in outputs {
        psbt.construct_output(address.script_pubkey(), value)
            .map_err(|_| BtcTransferError::WrongOutput(format!("{address}#{value}")))?;
    }

    psbt.complete_construction();
    Ok((psbt, change_final.sats()))
}

pub async fn drain_psbt(
    btc_account: &mut BtcAccount,
    fee_rate: f32,
    transfer_addr: Address,
    transfer_value: Sats,
    utxos_inputs: DrainUtxoInputs,
) -> Result<(Psbt, u64), BtcTransferError> {
    let spend_total = transfer_value;

    // Inputs
    if utxos_inputs.is_empty() {
        return Err(BtcTransferError::Tx(ConstructionError::NoInputs));
    };

    let total_balance = utxos_inputs
        .iter()
        .flat_map(|(_, data)| data.iter().map(|(x, _)| x.value))
        .sum();
    if total_balance < spend_total {
        return Err(BtcTransferError::Tx(
            ConstructionError::OutputExceedsInputs {
                input_value: total_balance,
                output_value: spend_total,
            },
        ));
    }

    // Outputs
    // rgb fun output
    let mut outputs = vec![];
    outputs.push((transfer_addr, spend_total));

    // PSBT
    let seqno = SeqNo::from_consensus_u32(0);
    let mut psbt = Psbt::create(PsbtVer::V2);

    // Inputs
    for (descr, data) in utxos_inputs {
        for (utxo_input, spk) in data {
            let WalletUtxo {
                outpoint,
                value,
                terminal,
                ..
            } = utxo_input;
            let prev_out = Prevout::new(outpoint, value);

            psbt.construct_input(prev_out, &descr, terminal, spk, seqno)
                .map_err(|_| BtcTransferError::WrongInput(format!("{outpoint}#{value}")))?;
        }
    }

    // Fee
    let mut fee_rate_psbt = psbt.clone();
    for (address, value) in outputs.clone() {
        fee_rate_psbt
            .construct_output(address.script_pubkey(), value)
            .map_err(|_| BtcTransferError::WrongOutput(format!("{address}#{value}")))?;
    }

    let fee = btc_account
        .wallet
        .fee_estimate(fee_rate_psbt, fee_rate_from_sat_vb(fee_rate))
        .expect("bp transfer");

    let last_index = outputs.iter().position(|(addr, _)| {
        addr.payload.script_pubkey() == transfer_addr.payload.script_pubkey()
    });

    let last_index = if let Some(last_index) = last_index {
        last_index
    } else {
        return Err(BtcTransferError::NoOutput);
    };

    let fee = Sats(fee);
    let change_final = spend_total.checked_sub(fee).ok_or(BtcTransferError::Tx(
        ConstructionError::NoFundsForFee {
            input_value: total_balance,
            output_value: spend_total,
            fee,
        },
    ))?;

    outputs.remove(last_index);
    outputs.insert(last_index, (transfer_addr, change_final));

    for (address, value) in outputs {
        psbt.construct_output(address.script_pubkey(), value)
            .map_err(|_| BtcTransferError::WrongOutput(format!("{address}#{value}")))?;
    }

    psbt.complete_construction();
    Ok((psbt, change_final.sats()))
}
