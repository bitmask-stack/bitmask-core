use crate::debug;
use crate::rgb::data::{RGBStock, RGBStockCrossStore};
use crate::rgb::mapper::AssetSchema;
use crate::rgb::models::{ContractAmount, ContractKit};
use crate::rgb::prefetch::BitmaskResolver;
use crate::rgb::swap::{AnchorShard, RgbExtraShard, RgbShard};
use crate::wallet::data::{BPWallet, SyncProvider};
use amplify::confinement::{Confined, U24};
use bitcoin::{FeeRate, Weight, psbt::Psbt as PsbtV0};
use bpstd::Psbt;
use bpstd::psbt::{KeyMap, PropKey};
use bpstd::{Descriptor, Keychain, Sats, ScriptPubkey, Vout, psbt};
use bpwallet::psbt::PsbtConstructor;
use bpwallet::psbt::{Beneficiary as BpBeneficiary, ConstructionError, PsbtMeta};
use bpwallet::{TxStatus, Txid};
use chrono::Utc;
use dbc::tapret::TapretCommitment;
use nonasync::persistence::CloneNoPersistence;
use psrgbt::{EmbedError, PSBT_RGB_PREFIX_STR, RgbOutExt, RgbPsbtError, RgbPsbtExt};
use psrgbt::{OpretKeyError, TapretKeyError};
use rgb::containers::ConsignmentExt;
use rgb::{
    CompletionError, CompositionError, DescriptorRgb, PayError, TransferParams, WalletProvider,
};
use rgbcore::SecretSeal;
use rgbcore::seals::txout::CloseMethod;
use rgbcore::vm::WitnessOrd;
use rgbcore::{
    AssignmentType, ChainNet, ContractId, GraphSeal, KnownTransition, Operation, Opout, OutputSeal,
    RevealedData, RevealedValue, TransitionBundle, TypedAssigns,
};
use rgbinvoice::{Allocation, Amount, Beneficiary, InvoiceState, NonFungible, RgbInvoice};
use rgbstd::MergeReveal;
use rgbstd::containers::{Batch, BuilderSeal, Transfer};
use rgbstd::contract::{
    AllocatedState, AssignmentsFilter, BuilderError, ContractError, FilterIncludeAll,
};
use rgbstd::persistence::{
    ContractStateWrite, IndexProvider, IndexWriteProvider, StashInconsistency, StashProvider,
    StashReadProvider, StashWriteProvider, StateProvider, StateReadProvider, StateWriteProvider,
    Stock,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ops::AddAssign;
use std::str::FromStr;

// Dust operation for segwit
const DUST_LIMIT_SATOSHI: u64 = 546;

// Base weight of a Txin, not counting the weight needed for satisfying it.
// prev_txid (32 bytes) + prev_vout (4 bytes) + sequence (4 bytes)
const TXIN_BASE_WEIGHT: usize = (32 + 4 + 4) * 4;

struct ContractFilters {
    contract_id: ContractId,
    stock: RGBStock,
    wallet: BPWallet,
}

impl AssignmentsFilter for ContractFilters {
    fn should_include(
        &self,
        outpoint: impl Into<rgbstd::Outpoint>,
        _: Option<rgbstd::Txid>,
    ) -> bool {
        let rgb_out = outpoint.into();
        let bp_out = crate::rgb::compat::outpoint_bitcoin_to_bp(rgb_out);

        // The allocation is usable only if *both* conditions hold:
        // 1. The wallet knows (and can spend) the anchoring UTXO.
        // 2. The stock has contract data for this outpoint.
        if self.wallet.utxo(bp_out).is_none() {
            return false;
        }

        self.stock
            .contract_assignments_for(self.contract_id, [rgb_out])
            .is_ok()
    }
}

#[allow(clippy::large_enum_variant)] // RGB 0.11 Pay/Composition errors dominate this product enum
#[allow(clippy::result_large_err)] // WalletExt returns this graph; do not reshape
#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum WalletErrors {
    /// wallet data inconsistency
    WalletDataInconsistency,
    /// just created
    JustCreated,
    /// PSBT without beneficiary address.
    NoBeneficiary,
    /// PSBT must be modifiable at this stage. {0}
    Unmodifiable(String),
    /// composition error: {0}
    CompositionError(#[from] CompositionError),
    /// rgb stock error. {0}
    StockError(String),
    /// completion error. {0}
    CompletionError(#[from] CompletionError),
    /// completion error. {0}
    PayError(#[from] PayError),
    /// contract error. {0}
    ContractError(#[from] ContractError),
    /// construction error. {0}
    ConstructionError(#[from] ConstructionError),
    /// embed error. {0}
    EmbedError(#[from] EmbedError),
    /// tapret key error. {0}
    TapretKeyError(#[from] TapretKeyError),
    /// tapret key error. {0}
    OpretKeyError(#[from] OpretKeyError),
    /// runtime error. {0}
    RuntimeError(#[from] String),
    /// Invalid pbst. '{0}'
    WrongPsbt(String),
}

#[allow(clippy::result_large_err)] // WalletErrors is the product Result
pub trait WalletExt {
    fn partial_psbt(
        &mut self,
        rgb_stock: &mut RGBStock,
        invoice: &RgbInvoice,
        method: CloseMethod,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), WalletErrors>;

    fn fee_estimate(&mut self, psbt: Psbt, fee_rate: FeeRate) -> Result<u64, WalletErrors>;

    fn finish_psbt_rgb(
        &mut self,
        rgb_stock: &mut RGBStock,
        invoice: &RgbInvoice,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), WalletErrors>;
}

impl WalletExt for BPWallet {
    fn partial_psbt(
        &mut self,
        rgb_stock: &mut RGBStock,
        invoice: &RgbInvoice,
        method: CloseMethod,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), WalletErrors> {
        let contract_id = invoice.contract.ok_or(CompositionError::NoContract)?;

        let contract = rgb_stock
            .contract_data(contract_id)
            .map_err(|e| WalletErrors::StockError(e.to_string()))?;

        let Some(ref assignment_state) = invoice.assignment_state else {
            return Err(WalletErrors::CompositionError(
                CompositionError::NoAssignmentState,
            ));
        };

        let invoice_assignment_type = invoice
            .assignment_name
            .as_ref()
            .map(|n| contract.schema.assignment_type(n.clone()));
        let assignment_type = invoice_assignment_type
            .as_ref()
            .or_else(|| {
                let assignment_types = contract
                    .schema
                    .assignment_types_for_state(assignment_state.clone().into());
                if assignment_types.len() == 1 {
                    Some(assignment_types[0])
                } else {
                    contract
                        .schema
                        .default_assignment
                        .as_ref()
                        .filter(|&assignment| assignment_types.contains(&assignment))
                }
            })
            .ok_or(CompositionError::NoAssignmentType)?;

        let outputs = match invoice.assignment_state.clone() {
            Some(InvoiceState::Amount(amount)) => {
                let filter = ContractFilters {
                    contract_id,
                    stock: rgb_stock.clone_no_persistence(),
                    wallet: self.clone_no_persistence(),
                };
                let mut state = contract
                    .fungible_raw(*assignment_type, &filter)?
                    .collect::<Vec<_>>();
                state.sort_by_key(|a| a.state);
                let mut sum = Amount::ZERO;
                state
                    .iter()
                    .rev()
                    .take_while(|a| {
                        if sum >= amount {
                            false
                        } else {
                            sum += a.state;
                            true
                        }
                    })
                    .map(|a| a.seal)
                    .collect::<Vec<_>>()
            }
            Some(InvoiceState::Data(NonFungible::FractionedToken(allocation))) => {
                let filter = ContractFilters {
                    contract_id,
                    stock: rgb_stock.clone_no_persistence(),
                    wallet: self.clone_no_persistence(),
                };
                let state = contract
                    .data_raw(*assignment_type, &filter)?
                    .collect::<Vec<_>>();

                let data_state = allocation.into();
                state
                    .into_iter()
                    .filter(|x| x.state == data_state)
                    .map(|x| x.seal)
                    .collect()
            }
            _ => {
                return Err(WalletErrors::CompositionError(
                    CompositionError::Unsupported,
                ));
            }
        };
        let beneficiaries = match invoice.beneficiary.into_inner() {
            Beneficiary::BlindedSeal(_) => vec![],
            Beneficiary::WitnessVout(pay2vout, _) => {
                vec![bpwallet::psbt::Beneficiary::new(
                    crate::rgb::compat::address_bp_from_pay2vout(
                        pay2vout,
                        invoice.address_network(),
                    ),
                    Sats::from_sats(params.min_amount),
                )]
            }
        };
        let mut outpoints: Vec<_> = outputs
            .iter()
            .map(|o| crate::rgb::compat::outpoint_bp_from_output_seal(*o))
            .collect();

        // Custom: Avoid dustless transaction
        let mut input_sats = Sats::ZERO;
        let total_sats_sended = Sats::from_sats(params.min_amount + params.tx.fee_sats);

        for outpoint in outpoints.clone() {
            let (utxo, _) = self
                .utxo(outpoint)
                .ok_or(WalletErrors::WalletDataInconsistency)?;

            input_sats += utxo.value;
        }

        let dust_limit = Sats::from_sats(DUST_LIMIT_SATOSHI);
        if input_sats < total_sats_sended + dust_limit {
            let bitcoin_keychains = [Keychain::OUTER, Keychain::INNER];

            let total = total_sats_sended + dust_limit;
            let mut selected = input_sats;
            let mut bitcoin_utxos: Vec<_> = self.utxos().collect();
            bitcoin_utxos.sort_by_key(|a| a.value);

            let bitcoin_utxos: Vec<_> = bitcoin_utxos
                .into_iter()
                .filter(|x| bitcoin_keychains.contains(&x.terminal.keychain))
                .take_while(move |utxo| {
                    if selected <= total {
                        selected.add_assign(utxo.value);
                        true
                    } else {
                        false
                    }
                })
                .map(|utxo| utxo.outpoint)
                .collect();

            outpoints.extend(bitcoin_utxos);
        }

        let mut params = params.clone();
        params.tx.change_keychain = crate::rgb::compat::rgb_close_method_keychain_u8(matches!(
            method,
            CloseMethod::TapretFirst
        ));
        let (mut psbt, meta) = self.construct_psbt(outpoints, &beneficiaries, params.tx.into())?;

        psbt.complete_construction();
        Ok((psbt, meta))
    }

    fn fee_estimate(&mut self, psbt: Psbt, fee_rate: FeeRate) -> Result<u64, WalletErrors> {
        // TODO: Over-simplification of bdk fee calculation. We need revise!
        // https://github.com/bitcoindevkit/bdk/blob/2867e88b64b4a8cf7136cc562ec61c077737a087/crates/bdk/src/wallet/mod.rs#L1009-L1131
        // https://github.com/bitcoindevkit/bdk/blob/2867e88b64b4a8cf7136cc562ec61c077737a087/crates/bdk/src/wallet/coin_selection.rs#L398-L630

        let psbt_legacy = PsbtV0::from_str(&psbt.to_base64_ver(bpstd::PsbtVer::V0))
            .map_err(|op| WalletErrors::WrongPsbt(op.to_string()))?;
        let mut fee = fee_rate
            .fee_wu(psbt_legacy.unsigned_tx.weight())
            .map(|a| a.to_sat())
            .unwrap_or(0);
        fee += fee_rate
            .fee_wu(Weight::from_wu(TXIN_BASE_WEIGHT as u64))
            .map(|a| a.to_sat())
            .unwrap_or(0)
            * psbt.inputs().count() as u64;
        fee += fee_rate
            .fee_wu(Weight::from_wu(2))
            .map(|a| a.to_sat())
            .unwrap_or(0);

        Ok(fee)
    }

    fn finish_psbt_rgb(
        &mut self,
        _: &mut RGBStock,
        _: &RgbInvoice,
        _: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), WalletErrors> {
        todo!()
    }
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ShardErrors {
    /// fascia error. {0}
    Fascia(String),
    /// transition compose error. {0}
    Compose(String),
    Pay(String),
    Builder(String),
    Parse(String),
}

pub trait StockExt {
    fn consume_bundle_raw(
        &mut self,
        contract_id: ContractId,
        bundle: TransitionBundle,
        witness_id: Txid,
        provider: &mut RGBStockCrossStore,
    ) -> Result<(), ShardErrors>;

    fn consume_anchor_raw(
        &mut self,
        contract_id: ContractId,
        anchor: AnchorShard,
        witness_id: Txid,
        provider: &mut RGBStockCrossStore,
    ) -> Result<(), ShardErrors>;
}

impl StockExt for RGBStock {
    fn consume_bundle_raw(
        &mut self,
        contract_id: ContractId,
        bundle: TransitionBundle,
        witness_id: Txid,
        provider: &mut RGBStockCrossStore,
    ) -> Result<(), ShardErrors> {
        let witness_id = crate::rgb::compat::txid_bp_to_bitcoin(witness_id);
        let mut stash = self.as_stash_provider().clone_no_persistence();
        let mut state = self.as_state_provider().clone_no_persistence();
        let mut index = self.as_index_provider().clone_no_persistence();

        //  index.index_bundle(contract_id, &bundle, witness_id)?;
        let bundle_id = bundle.bundle_id();

        if state.contract_state(contract_id).is_ok() {
            index
                .register_bundle(bundle_id, witness_id, contract_id)
                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;

            for KnownTransition { opid, transition } in &bundle.clone().known_transitions {
                index
                    .register_operation(*opid, bundle_id)
                    .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                for input in &transition.inputs {
                    index
                        .register_spending(input.op, bundle_id)
                        .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                }

                for (type_id, assign) in transition.assignments.iter() {
                    match assign {
                        TypedAssigns::Declarative(vec) => {
                            index
                                .index_transition_assignments(
                                    contract_id,
                                    vec,
                                    *opid,
                                    *type_id,
                                    witness_id,
                                )
                                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                        }
                        TypedAssigns::Fungible(vec) => {
                            index
                                .index_transition_assignments(
                                    contract_id,
                                    vec,
                                    *opid,
                                    *type_id,
                                    witness_id,
                                )
                                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                        }
                        TypedAssigns::Structured(vec) => {
                            index
                                .index_transition_assignments(
                                    contract_id,
                                    vec,
                                    *opid,
                                    *type_id,
                                    witness_id,
                                )
                                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                        }
                    }
                }
            }

            // state.update_from_bundle(contract_id, &bundle, witness_id, &resolver)?;
            for known_transition in bundle.clone().known_transitions {
                let transition = known_transition.transition.clone();
                if let Some(mut contract_state) = state
                    .update_contract(transition.contract_id)
                    .expect("invalid contract state")
                {
                    let ord = WitnessOrd::Tentative;
                    contract_state
                        .add_transition(&transition, witness_id, ord, bundle_id)
                        .expect("invalid transition");
                }
            }

            // stash.consume_bundle(bundle)?;
            let bundle = match stash.bundle(bundle_id) {
                Ok(b) => {
                    let mut prev_bundle = b.clone();
                    prev_bundle
                        .merge_reveal(&bundle)
                        .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                    prev_bundle
                }
                Err(_) => bundle,
            };

            stash
                .replace_bundle(bundle)
                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;

            let mut cp = RGBStock::with(
                stash.clone_no_persistence(),
                state.clone_no_persistence(),
                index.clone_no_persistence(),
            );

            cp.sync(provider).expect("invalid sync");

            // Debug: verify allocations now point to the intended witness outpoint for this contract
            if let Ok(latest) = RGBStock::get(provider.clone())
                && let Ok(cdata) = latest.contract_data(contract_id)
            {
                let outs: Vec<String> = cdata
                    .allocations(&FilterIncludeAll)
                    .map(|a| a.seal.to_string())
                    .collect();
                debug!(format!(
                    "ext.consume_bundle_raw indexed contract_id={} witness_id={} allocations_now=[{}]",
                    contract_id,
                    witness_id,
                    outs.join(",")
                ));
            }
        }

        Ok(())
    }

    fn consume_anchor_raw(
        &mut self,
        contract_id: ContractId,
        anchor: AnchorShard,
        witness_id: Txid,
        provider: &mut RGBStockCrossStore,
    ) -> Result<(), ShardErrors> {
        let witness_id = crate::rgb::compat::txid_bp_to_bitcoin(witness_id);
        let mut stash = self.as_stash_provider().clone_no_persistence();
        let mut state = self.as_state_provider().clone_no_persistence();
        let mut index = self.as_index_provider().clone_no_persistence();

        let AnchorShard {
            bundle,
            witness_seal,
        } = anchor;

        //  index.index_bundle(contract_id, &bundle, witness_id)?;
        let bundle_id = bundle.bundle_id();

        if state.contract_state(contract_id).is_ok() {
            index
                .register_bundle(bundle_id, witness_id, contract_id)
                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;

            for KnownTransition { opid, transition } in &bundle.clone().known_transitions {
                index
                    .register_operation(*opid, bundle_id)
                    .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                for input in &transition.inputs {
                    index
                        .register_spending(input.op, bundle_id)
                        .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                }

                for (type_id, assign) in transition.assignments.iter() {
                    match assign {
                        TypedAssigns::Declarative(vec) => {
                            index
                                .index_transition_assignments(
                                    contract_id,
                                    vec,
                                    *opid,
                                    *type_id,
                                    witness_id,
                                )
                                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                        }
                        TypedAssigns::Fungible(vec) => {
                            index
                                .index_transition_assignments(
                                    contract_id,
                                    vec,
                                    *opid,
                                    *type_id,
                                    witness_id,
                                )
                                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                        }
                        TypedAssigns::Structured(vec) => {
                            index
                                .index_transition_assignments(
                                    contract_id,
                                    vec,
                                    *opid,
                                    *type_id,
                                    witness_id,
                                )
                                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                        }
                    }
                }
            }

            // state.update_from_bundle(contract_id, &bundle, witness_id, &resolver)?;
            for known_transition in bundle.clone().known_transitions {
                let transition = known_transition.transition.clone();
                if let Some(mut contract_state) = state
                    .update_contract(transition.contract_id)
                    .expect("invalid contract state")
                {
                    let ord = WitnessOrd::Tentative;
                    contract_state
                        .add_transition(&transition, witness_id, ord, bundle_id)
                        .expect("invalid transition");
                }
            }

            // stash.consume_bundle(bundle)?;
            let bundle = match stash.bundle(bundle_id) {
                Ok(b) => {
                    let mut prev_bundle = b.clone();
                    prev_bundle
                        .merge_reveal(&bundle)
                        .map_err(|err| ShardErrors::Fascia(err.to_string()))?;
                    prev_bundle
                }
                Err(_) => bundle,
            };

            stash
                .replace_bundle(bundle)
                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;

            stash
                .replace_witness(witness_seal)
                .map_err(|err| ShardErrors::Fascia(err.to_string()))?;

            let mut cp = RGBStock::with(
                stash.clone_no_persistence(),
                state.clone_no_persistence(),
                index.clone_no_persistence(),
            );

            cp.sync(provider).expect("invalid sync");
        }

        Ok(())
    }
}

/**
 Derive fungible beneficiary amount from a consignment.

 Heuristic v1:
 - Iterate across all known transitions of all bundles inside the Transfer.
 - Collect all positive fungible assignment amounts.
 - Choose the smallest positive as the beneficiary payment (larger ones are assumed to be change).

 Notes:
 - This augments display metadata only; it does not mutate RGB state.
 - For UDA (RGB21) or unexpected structures, it returns None.
 - This function is intentionally conservative; future refinements may discriminate change vs beneficiary via seal type.
*/
pub(crate) fn derive_fungible_amount_from_consignment(transfer: &Transfer) -> Option<u64> {
    // Phase 1: instrumentation to confirm assignment shapes at runtime.
    // After confirming the element fields for fungible assigns, we will replace this with
    // actual extraction and "smallest positive" selection.
    debug!(format!(
        "[derive] start transfer_id={} contract_id={} schema_id={}",
        transfer.consignment_id(),
        transfer.contract_id(),
        transfer.schema_id()
    ));

    let mut saw_any_fungible = false;
    let mut bundle_idx = 0usize;
    for bundle_ref in &transfer.bundles {
        bundle_idx += 1;
        let bundle = &bundle_ref.bundle;
        debug!(format!(
            "[derive] bundle {} id={} known_transitions={}",
            bundle_idx,
            bundle.bundle_id(),
            bundle.known_transitions.len()
        ));
        for (ti, known) in bundle.known_transitions.iter().enumerate() {
            let transition = &known.transition;
            let assigns_len = transition.assignments.len();
            debug!(format!(
                "[derive]  transition {} assignments={}",
                ti, assigns_len
            ));
            for (type_id, assigns) in transition.assignments.iter() {
                match assigns {
                    TypedAssigns::Fungible(vec) => {
                        saw_any_fungible = true;
                        debug!(format!(
                            "[derive]   fungible assigns: type_id={} count={} sample={:?}",
                            type_id,
                            vec.len(),
                            vec.first()
                        ));
                    }
                    TypedAssigns::Declarative(vec) => {
                        debug!(format!(
                            "[derive]   declarative assigns: type_id={} count={}",
                            type_id,
                            vec.len()
                        ));
                    }
                    TypedAssigns::Structured(vec) => {
                        debug!(format!(
                            "[derive]   structured assigns: type_id={} count={}",
                            type_id,
                            vec.len()
                        ));
                    }
                }
            }
        }
    }

    debug!(format!(
        "[derive] result return={:?} saw_any_fungible={}",
        None::<u64>, saw_any_fungible
    ));
    None
}

#[allow(clippy::result_large_err)] // PayError is RGB 0.11; do not reshape
pub trait PsbtExt {
    fn pay_buyer(
        &mut self,
        rgb_invoice: RgbInvoice,
        rgb_shard: RgbShard,
        rgb_stock: &mut RGBStock,
    ) -> Result<SecretSeal, PayError>;

    fn pay_seller(
        &mut self,
        offer_seal: SecretSeal,
        contract_kit: ContractKit,
        contract_amount: ContractAmount,
        rgb_shard: RgbShard,
        rgb_stock: &mut RGBStock,
    ) -> Result<(), PayError>;

    fn add_extra_buyer(
        &mut self,
        chain_net: ChainNet,
        rgb_extra_shards: Vec<RgbExtraShard>,
        rgb_stock: &mut RGBStock,
        rgb_resolver: &mut BitmaskResolver,
    ) -> Result<(), PayError>;

    fn add_extra_seller(
        &mut self,
        chain_net: ChainNet,
        rgb_extra_shards: Vec<RgbExtraShard>,
        rgb_stock: &mut RGBStock,
        rgb_resolver: &mut BitmaskResolver,
    ) -> Result<(), PayError>;

    fn export_commit(&self) -> Result<TapretCommitment, TapretKeyError>;
}

impl PsbtExt for Psbt {
    fn pay_buyer(
        &mut self,
        rgb_invoice: RgbInvoice,
        rgb_shard: RgbShard,
        rgb_stock: &mut RGBStock,
    ) -> Result<SecretSeal, PayError> {
        let Some(ref assignment_state) = rgb_invoice.assignment_state else {
            return Err(PayError::Composition(CompositionError::NoAssignmentState));
        };

        let contract_id = rgb_invoice.contract.ok_or(CompositionError::NoContract)?;
        let contract = rgb_stock
            .contract_data(contract_id)
            .map_err(|_| PayError::Composition(CompositionError::NoContract))?;

        let invoice_assignment_type = rgb_invoice
            .assignment_name
            .as_ref()
            .map(|n| contract.schema.assignment_type(n.clone()));
        let assignment_type = invoice_assignment_type
            .as_ref()
            .or_else(|| {
                let assignment_types = contract
                    .schema
                    .assignment_types_for_state(assignment_state.clone().into());
                if assignment_types.len() == 1 {
                    Some(assignment_types[0])
                } else {
                    contract
                        .schema
                        .default_assignment
                        .as_ref()
                        .filter(|&assignment| assignment_types.contains(&assignment))
                }
            })
            .ok_or(CompositionError::NoAssignmentType)?;
        let transition_type = contract
            .schema
            .default_transition_for_assignment(assignment_type);

        let secret_seal = match rgb_invoice.beneficiary.into_inner() {
            Beneficiary::BlindedSeal(seal) => seal,
            Beneficiary::WitnessVout(addr, _) => {
                let s = crate::rgb::compat::script_pubkey_bp_from_pay2vout(addr);
                let vout = self
                    .outputs()
                    .position(|output| output.script == s)
                    .unwrap();
                let vout = Vout::from_u32(vout as u32);

                let seal = GraphSeal::new_random_vout(crate::rgb::compat::vout_bp_to_rgb(vout));
                rgb_stock.store_secret_seal(seal).expect("rgb extra");
                seal.to_secret_seal()
            }
        };

        let builder_seal = BuilderSeal::Concealed(secret_seal);
        let mut main_builder = rgb_stock
            .transition_builder_raw(contract_id, transition_type)
            .map_err(|e| e.to_string())
            .expect("invalid transition data");

        for (op, state) in rgb_shard.inputs {
            main_builder = main_builder
                .add_input(op, state.clone())
                .expect("invalid transition data");
        }

        // Add payments to beneficiary and change
        let change_state = self
            .inputs()
            .flat_map(|x| x.proprietary.clone())
            .find(|(key, _)| key == &PropKey::rgb_offer_input(contract_id))
            .map(|(_, x)| x);

        match assignment_state.clone() {
            InvoiceState::Amount(amt) => {
                main_builder = main_builder
                    .add_fungible_state_raw(*assignment_type, builder_seal, amt)
                    .expect("invalid fungible transition");

                if let Some(change_state) = change_state.clone() {
                    let seal = GraphSeal::new_random_vout(0);
                    let change_bytes = change_state.as_slice();
                    let change_value = u64::from_be_bytes(change_bytes.try_into().unwrap());

                    if change_value > 0 {
                        let change_state = Amount::from(change_value);
                        let contract_state =
                            AllocatedState::Amount(RevealedValue::from(change_state));

                        main_builder = main_builder
                            .add_owned_state_raw(*assignment_type, seal, contract_state)
                            .unwrap();
                    }
                }
            }
            InvoiceState::Data(data) => match data {
                NonFungible::FractionedToken(allocation) => {
                    let lookup_state = RevealedData::from(allocation);
                    main_builder = main_builder
                        .add_data_raw(*assignment_type, builder_seal, lookup_state)
                        .expect("invalid nft transition");
                }
            },
            InvoiceState::Void => {
                main_builder = main_builder
                    .add_rights_raw(*assignment_type, builder_seal)
                    .expect("invalid void transition");
            }
        }

        let transition = main_builder
            .complete_transition()
            .expect("invalid transition");

        self.push_rgb_transition(transition).unwrap();

        Ok(secret_seal)
    }

    fn add_extra_buyer(
        &mut self,
        chain_net: ChainNet,
        rgb_extra_shards: Vec<RgbExtraShard>,
        rgb_stock: &mut RGBStock,
        rgb_resolver: &mut BitmaskResolver,
    ) -> Result<(), PayError> {
        if rgb_extra_shards.is_empty() {
            return Ok(());
        }

        // extra state - create and store seal so buyer wallet can recognize outputs later
        let seal = GraphSeal::new_random_vout(1);
        rgb_stock
            .store_secret_seal(seal)
            .expect("failed to store secret seal");
        let next_seal = BuilderSeal::Revealed(seal);

        for rgb_extra_shard in rgb_extra_shards {
            let RgbExtraShard {
                contract_id,
                contract,
                inputs,
                ..
            } = rgb_extra_shard.clone();
            let type_system = AssetSchema::from(contract.schema_id()).types();
            let contract = contract
                .validate(
                    rgb_resolver,
                    &crate::rgb::compat::validation_config(chain_net, type_system),
                )
                .expect("invalid contract");

            rgb_stock
                .import_contract(contract.clone(), rgb_resolver.clone())
                .expect("invalid contract");

            // FIX: Create ONE transition per contract with ALL inputs
            // Previous code created multiple transitions per contract (one per assignment type)
            // which caused contracts to be missing from tapret anchors
            if inputs.is_empty() {
                continue;
            }

            // Get transition type from the first input's assignment type
            let first_opout = inputs.keys().next().expect("inputs is non-empty");
            let transition_type = contract
                .schema
                .default_transition_for_assignment(&first_opout.ty);

            let mut extra_builder = rgb_stock
                .transition_builder_raw(contract_id, transition_type)
                .expect("invalid transition");

            // Add ALL inputs to the single transition
            for (opout, state) in inputs.iter() {
                extra_builder = extra_builder
                    .add_input(*opout, state.clone())
                    .expect("invalid transition data");
            }

            // Group and sum amounts by assignment type for fungible assets (RGB20)
            // Non-fungible assets (RGB21) are added individually
            let mut amounts_by_type: BTreeMap<AssignmentType, u64> = BTreeMap::new();
            let mut non_fungible_states: Vec<(AssignmentType, AllocatedState)> = Vec::new();

            for (opout, state) in inputs.iter() {
                match state {
                    AllocatedState::Amount(revealed_value) => {
                        // Fungible asset - sum the amounts
                        *amounts_by_type.entry(opout.ty).or_insert(0) += revealed_value.as_u64();
                    }
                    AllocatedState::Data(_) => {
                        // Non-fungible asset - keep separate
                        non_fungible_states.push((opout.ty, state.clone()));
                    }
                    AllocatedState::Void => {
                        // Void state - skip
                    }
                }
            }

            // Add summed fungible amounts as outputs
            for (assignment_type, total_amount) in amounts_by_type.iter() {
                debug!(format!(
                    "add_extra_buyer.sum_fungible contract={} type={} total_amount={}",
                    contract_id, assignment_type, total_amount
                ));
                let amount_state = Amount::from(*total_amount);
                let allocated = AllocatedState::Amount(RevealedValue::from(amount_state));
                extra_builder = extra_builder
                    .add_owned_state_raw(*assignment_type, next_seal, allocated)
                    .unwrap();
            }

            // Add non-fungible states individually
            for (assignment_type, state) in non_fungible_states {
                extra_builder = extra_builder
                    .add_owned_state_raw(assignment_type, next_seal, state)
                    .unwrap();
            }

            let transition = extra_builder
                .complete_transition()
                .expect("invalid transition");

            let opid = transition.id();

            debug!(format!(
                "add_extra_buyer.before_consumer_registration contract={} opid={} inputs_count={}",
                contract_id,
                opid,
                transition.inputs().len()
            ));

            // CRITICAL FIX: Set RGB contract consumer info before pushing transition
            // This registers each input opout as consumed by this transition,
            // which allows rgb_commit() -> rgb_bundles_to_mpc() -> rgb_contract_ids()
            // to find the contract and include it in the tapret commitment
            for opout in transition.inputs() {
                let _ = self.set_rgb_contract_consumer(contract_id, opout, opid);
            }

            debug!(format!(
                "add_extra_buyer.before_push contract={} opid={}",
                contract_id, opid
            ));

            // Push the transition to PSBT
            let push_result = self.push_rgb_transition(transition);

            debug!(format!(
                "add_extra_buyer.after_push contract={} opid={} result={:?}",
                contract_id, opid, push_result
            ));

            // Verify the transition was stored and can be retrieved
            match self.rgb_transition(opid) {
                Ok(Some(_)) => {
                    debug!(format!(
                        "add_extra_buyer.transition_verified contract={} opid={}",
                        contract_id, opid
                    ));
                }
                Ok(None) => {
                    debug!(format!(
                        "add_extra_buyer.transition_NOT_FOUND contract={} opid={}",
                        contract_id, opid
                    ));
                }
                Err(e) => {
                    debug!(format!(
                        "add_extra_buyer.transition_retrieval_error contract={} opid={} error={:?}",
                        contract_id, opid, e
                    ));
                }
            }

            push_result.unwrap();
        }

        Ok(())
    }

    fn pay_seller(
        &mut self,
        offer_seal: SecretSeal,
        contract_kit: ContractKit,
        contract_amount: ContractAmount,
        rgb_shard: RgbShard,
        rgb_stock: &mut RGBStock,
    ) -> Result<(), PayError> {
        let is_uda = contract_kit.info.contract_type().is_uda();
        let contract_id = contract_kit.info.contract_id();
        let contract = rgb_stock
            .contract_data(*contract_id)
            .map_err(|e| e.to_string())
            .expect("invalid contract data");

        let assignment_type = contract.schema.default_assignment.unwrap();

        let transition_type = contract
            .schema
            .default_transition_for_assignment(&assignment_type);

        let mut main_builder = rgb_stock
            .transition_builder_raw(*contract_id, transition_type)
            .map_err(|e| e.to_string())
            .expect("invalid transition data");

        for (op, state) in rgb_shard.inputs {
            main_builder = main_builder
                .add_input(op, state.clone())
                .expect("invalid transition data");
        }

        // Add payments to beneficiary and change
        let change_state = self
            .inputs()
            .flat_map(|x| x.proprietary.clone())
            .find(|(key, _)| key == &PropKey::rgb_bid_input(*contract_id))
            .map(|(_, x)| x);

        let builder_seal = BuilderSeal::Concealed(offer_seal);

        if is_uda {
            let allocation = Allocation::with(1, 1);
            let lookup_state = RevealedData::from(allocation);
            main_builder = main_builder
                .add_data_raw(assignment_type, builder_seal, lookup_state)
                .expect("invalid nft transition");
        } else {
            main_builder = main_builder
                .add_fungible_state_raw(assignment_type, builder_seal, contract_amount.to_value())
                .expect("invalid fungible transition");

            if let Some(change_state) = change_state {
                // TODO: change to find first buyer address index instead of fized index
                let seal = GraphSeal::new_random_vout(1);
                let change_bytes = change_state.as_slice();
                let change_value = u64::from_be_bytes(change_bytes.try_into().unwrap());

                if change_value > 0 {
                    let change_state = Amount::from(change_value);
                    let contract_state = AllocatedState::Amount(RevealedValue::from(change_state));

                    main_builder = main_builder
                        .add_owned_state_raw(assignment_type, seal, contract_state)
                        .unwrap();
                }
            }
        }

        let transition = main_builder
            .complete_transition()
            .expect("invalid transition");

        self.push_rgb_transition(transition).unwrap();

        Ok(())
    }

    fn add_extra_seller(
        &mut self,
        chain_net: ChainNet,
        rgb_extra_shards: Vec<RgbExtraShard>,
        rgb_stock: &mut RGBStock,
        rgb_resolver: &mut BitmaskResolver,
    ) -> Result<(), PayError> {
        // extra state - create and store seal so seller wallet can recognize outputs later
        let seal = GraphSeal::new_random_vout(0);
        rgb_stock
            .store_secret_seal(seal)
            .expect("failed to store secret seal");
        let next_seal = BuilderSeal::Revealed(seal);
        for rgb_extra_shard in rgb_extra_shards {
            let RgbExtraShard {
                contract_id,
                contract,
                inputs,
                ..
            } = rgb_extra_shard.clone();
            let type_system = AssetSchema::from(contract.schema_id()).types();
            let contract = contract
                .validate(
                    rgb_resolver,
                    &crate::rgb::compat::validation_config(chain_net, type_system),
                )
                .expect("invalid contract");

            rgb_stock
                .import_contract(contract.clone(), rgb_resolver.clone())
                .expect("invalid contract");

            // FIX: Create ONE transition per contract with ALL inputs
            // Previous code created multiple transitions per contract (one per assignment type)
            // which caused contracts to be missing from tapret anchors
            if inputs.is_empty() {
                continue;
            }

            // Get transition type from the first input's assignment type
            let first_opout = inputs.keys().next().expect("inputs is non-empty");
            let transition_type = contract
                .schema
                .default_transition_for_assignment(&first_opout.ty);

            let mut extra_builder = rgb_stock
                .transition_builder_raw(contract_id, transition_type)
                .expect("invalid transition");

            // Add ALL inputs to the single transition
            for (opout, state) in inputs.iter() {
                extra_builder = extra_builder
                    .add_input(*opout, state.clone())
                    .expect("invalid transition data");
            }

            // Group and sum amounts by assignment type for fungible assets (RGB20)
            // Non-fungible assets (RGB21) are added individually
            let mut amounts_by_type: BTreeMap<AssignmentType, u64> = BTreeMap::new();
            let mut non_fungible_states: Vec<(AssignmentType, AllocatedState)> = Vec::new();

            for (opout, state) in inputs.iter() {
                match state {
                    AllocatedState::Amount(revealed_value) => {
                        // Fungible asset - sum the amounts
                        *amounts_by_type.entry(opout.ty).or_insert(0) += revealed_value.as_u64();
                    }
                    AllocatedState::Data(_) => {
                        // Non-fungible asset - keep separate
                        non_fungible_states.push((opout.ty, state.clone()));
                    }
                    AllocatedState::Void => {
                        // Void state - skip
                    }
                }
            }

            // Add summed fungible amounts as outputs
            for (assignment_type, total_amount) in amounts_by_type.iter() {
                debug!(format!(
                    "add_extra_seller.sum_fungible contract={} type={} total_amount={}",
                    contract_id, assignment_type, total_amount
                ));
                let amount_state = Amount::from(*total_amount);
                let allocated = AllocatedState::Amount(RevealedValue::from(amount_state));
                extra_builder = extra_builder
                    .add_owned_state_raw(*assignment_type, next_seal, allocated)
                    .unwrap();
            }

            // Add non-fungible states individually
            for (assignment_type, state) in non_fungible_states {
                extra_builder = extra_builder
                    .add_owned_state_raw(assignment_type, next_seal, state)
                    .unwrap();
            }

            let transition = extra_builder
                .complete_transition()
                .expect("invalid transition");

            let opid = transition.id();

            debug!(format!(
                "add_extra_seller.before_consumer_registration contract={} opid={} inputs_count={}",
                contract_id,
                opid,
                transition.inputs().len()
            ));

            // CRITICAL FIX: Set RGB contract consumer info before pushing transition
            // This registers each input opout as consumed by this transition,
            // which allows rgb_commit() -> rgb_bundles_to_mpc() -> rgb_contract_ids()
            // to find the contract and include it in the tapret commitment
            for opout in transition.inputs() {
                let _ = self.set_rgb_contract_consumer(contract_id, opout, opid);
            }

            debug!(format!(
                "add_extra_seller.before_push contract={} opid={}",
                contract_id, opid
            ));

            // Push transition to PSBT
            let push_result = self.push_rgb_transition(transition);

            debug!(format!(
                "add_extra_seller.after_push contract={} opid={} result={:?}",
                contract_id, opid, push_result
            ));

            // Verify the transition was stored and can be retrieved
            match self.rgb_transition(opid) {
                Ok(Some(_)) => {
                    debug!(format!(
                        "add_extra_seller.transition_verified contract={} opid={}",
                        contract_id, opid
                    ));
                }
                Ok(None) => {
                    debug!(format!(
                        "add_extra_seller.transition_NOT_FOUND contract={} opid={}",
                        contract_id, opid
                    ));
                }
                Err(e) => {
                    debug!(format!(
                        "add_extra_seller.transition_retrieval_error contract={} opid={} error={:?}",
                        contract_id, opid, e
                    ));
                }
            }

            push_result.unwrap();
        }

        Ok(())
    }

    fn export_commit(&self) -> Result<TapretCommitment, TapretKeyError> {
        let outputs: Vec<_> = self.outputs().collect();
        let commit = outputs[0].tapret_commitment()?;
        TapretCommitment::from_str(&commit.to_string())
            .map_err(|_| TapretKeyError::InvalidCommitment)
    }
}

pub const PSBT_IN_RGB_OFFER_CHANGE: u64 = 0x98;

pub const PSBT_IN_RGB_BID_CHANGE: u64 = 0x99;

pub trait PsbtOrderInExt {
    #[allow(clippy::result_large_err)]
    fn rgb_offer_in(&self, contract_id: ContractId) -> bool;

    #[allow(clippy::result_large_err)]
    fn set_rgb_offer_in(
        &mut self,
        contract_id: ContractId,
        change: u64,
    ) -> Result<bool, RgbPsbtError>;

    #[allow(clippy::result_large_err)]
    fn rgb_bid_in(&self, contract_id: ContractId) -> bool;

    #[allow(clippy::result_large_err)]
    fn set_rgb_bid_in(
        &mut self,
        contract_id: ContractId,
        change: u64,
    ) -> Result<bool, RgbPsbtError>;
}

impl PsbtOrderInExt for psbt::Input {
    fn rgb_offer_in(&self, contract_id: ContractId) -> bool {
        self.proprietary
            .get(&PropKey::rgb_offer_input(contract_id))
            .is_some()
    }

    fn set_rgb_offer_in(
        &mut self,
        contract_id: ContractId,
        change: u64,
    ) -> Result<bool, RgbPsbtError> {
        let key = PropKey::rgb_offer_input(contract_id);
        self.insert_proprietary(key, change.to_be_bytes().to_vec().into());

        Ok(true)
    }

    fn rgb_bid_in(&self, contract_id: ContractId) -> bool {
        self.proprietary
            .get(&PropKey::rgb_bid_input(contract_id))
            .is_some()
    }

    fn set_rgb_bid_in(
        &mut self,
        contract_id: ContractId,
        change: u64,
    ) -> Result<bool, RgbPsbtError> {
        let key = PropKey::rgb_bid_input(contract_id);
        self.insert_proprietary(key, change.to_be_bytes().to_vec().into());

        Ok(true)
    }
}

pub trait PropRgbOrderKey {
    fn rgb_offer_input(contract_id: ContractId) -> PropKey {
        PropKey {
            identifier: PSBT_RGB_PREFIX_STR.to_owned(),
            subtype: PSBT_IN_RGB_OFFER_CHANGE,
            data: contract_id.to_vec().into(),
        }
    }

    fn rgb_bid_input(contract_id: ContractId) -> PropKey {
        PropKey {
            identifier: PSBT_RGB_PREFIX_STR.to_owned(),
            subtype: PSBT_IN_RGB_BID_CHANGE,
            data: contract_id.to_vec().into(),
        }
    }
}

impl PropRgbOrderKey for PropKey {}

#[allow(clippy::result_large_err)] // PayError / CompositionError are RGB 0.11
pub trait PayExt {
    #[allow(clippy::result_large_err)]
    fn make_pay<S: StashProvider, H: StateProvider, P: IndexProvider>(
        &mut self,
        stock: &mut Stock<S, H, P>,
        invoice: &RgbInvoice,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta, Transfer), PayError>;

    fn build_psbt_rgb<S: StashProvider, H: StateProvider, P: IndexProvider>(
        &mut self,
        stock: &Stock<S, H, P>,
        invoice: &RgbInvoice,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), CompositionError>;
}

impl PayExt for BPWallet {
    fn make_pay<S: StashProvider, H: StateProvider, P: IndexProvider>(
        &mut self,
        stock: &mut Stock<S, H, P>,
        invoice: &RgbInvoice,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta, Transfer), PayError> {
        let (mut psbt, meta) = self.build_psbt_rgb(stock, invoice, params)?;
        // ... here we pass PSBT around signers, if necessary
        let beneficiary_vout = match invoice.beneficiary.into_inner() {
            Beneficiary::WitnessVout(pay2vout, _) => {
                let s = crate::rgb::compat::script_pubkey_bp_from_pay2vout(pay2vout);
                psbt.outputs()
                    .find(|output| output.script == s)
                    .map(|output| output.vout().to_u32())
            }
            Beneficiary::BlindedSeal(_) => None,
        };
        let transfer = match self.transfer(stock, invoice, &mut psbt, beneficiary_vout) {
            Ok(transfer) => transfer,
            Err(e) => return Err(PayError::Completion(e)),
        };
        Ok((psbt, meta, transfer))
    }

    #[allow(clippy::result_large_err)]
    fn build_psbt_rgb<S: StashProvider, H: StateProvider, P: IndexProvider>(
        &mut self,
        stock: &Stock<S, H, P>,
        invoice: &RgbInvoice,
        mut params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), CompositionError> {
        let close_method = self.descriptor().close_method();

        let contract_id = invoice.contract.ok_or(CompositionError::NoContract)?;
        debug!(format!(
            "build_psbt_rgb.main_contract contract_id={} invoice_amount={:?}",
            contract_id, invoice.assignment_state
        ));
        let contract = stock
            .contract_data(contract_id)
            .map_err(|e| e.to_string())?;

        if let Some(invoice_schema) = invoice.schema
            && invoice_schema != contract.schema.schema_id()
        {
            return Err(CompositionError::InvalidSchema);
        }

        let contract_genesis = stock
            .as_stash_provider()
            .genesis(contract_id)
            .map_err(|_| CompositionError::UnknownContract)?;
        let contract_chain_net = contract_genesis.chain_net;
        let invoice_chain_net = invoice.chain_network();
        if contract_chain_net != invoice_chain_net {
            return Err(CompositionError::InvoiceBeneficiaryWrongChainNet(
                invoice_chain_net,
                contract_chain_net,
            ));
        }

        if let Some(expiry) = invoice.expiry
            && expiry < Utc::now().timestamp()
        {
            return Err(CompositionError::InvoiceExpired);
        }

        let Some(ref assignment_state) = invoice.assignment_state else {
            return Err(CompositionError::NoAssignmentState);
        };

        let invoice_assignment_type = invoice
            .assignment_name
            .as_ref()
            .map(|n| contract.schema.assignment_type(n.clone()));
        let assignment_type = invoice_assignment_type
            .as_ref()
            .or_else(|| {
                let assignment_types = contract
                    .schema
                    .assignment_types_for_state(assignment_state.clone().into());
                if assignment_types.len() == 1 {
                    Some(assignment_types[0])
                } else {
                    contract
                        .schema
                        .default_assignment
                        .as_ref()
                        .filter(|&assignment| assignment_types.contains(&assignment))
                }
            })
            .ok_or(CompositionError::NoAssignmentType)?;
        let transition_type = contract
            .schema
            .default_transition_for_assignment(assignment_type);

        let filter = ContractOutpointsFilter {
            contract_id,
            stock,
            wallet: self,
        };
        let prev_outputs = match assignment_state {
            InvoiceState::Amount(amount) => {
                let state: BTreeMap<_, Vec<Amount>> = contract
                    .fungible_raw(*assignment_type, &filter)?
                    .fold(bmap![], |mut set, a| {
                        set.entry(a.seal).or_default().push(a.state);
                        set
                    });
                let mut state: Vec<_> = state
                    .into_iter()
                    .map(|(seal, vals)| (vals.iter().copied().sum::<Amount>(), seal, vals))
                    .collect();
                state.sort_by_key(|(sum, _, _)| *sum);
                let mut sum = Amount::ZERO;
                let selection = state
                    .iter()
                    .rev()
                    .take_while(|(val, _, _)| {
                        if sum >= *amount {
                            false
                        } else {
                            sum += *val;
                            true
                        }
                    })
                    .map(|(_, seal, _)| *seal)
                    .collect::<BTreeSet<_>>();
                if sum < *amount { bset![] } else { selection }
            }
            InvoiceState::Data(NonFungible::FractionedToken(allocation)) => {
                let data_state = RevealedData::from(*allocation);
                contract
                    .data_raw(*assignment_type, &filter)?
                    .filter(|x| x.state == data_state)
                    .map(|x| x.seal)
                    .collect::<BTreeSet<_>>()
            }
            InvoiceState::Void => contract
                .rights_raw(*assignment_type, &filter)?
                .map(|x| x.seal)
                .collect::<BTreeSet<_>>(),
        };
        if prev_outputs.is_empty() {
            return Err(CompositionError::InsufficientState);
        }
        let prev_outpoints: Vec<_> = prev_outputs
            .iter()
            .map(|o| crate::rgb::compat::outpoint_bp_from_output_seal(*o))
            .collect();
        params.tx.change_keychain = crate::rgb::compat::rgb_close_method_keychain_u8(matches!(
            close_method,
            CloseMethod::TapretFirst
        ));

        let (beneficiaries, beneficiary_script) = match invoice.beneficiary.into_inner() {
            Beneficiary::BlindedSeal(_) => (vec![], None),
            Beneficiary::WitnessVout(pay2vout, _) => {
                let script = crate::rgb::compat::script_pubkey_bp_from_pay2vout(pay2vout);
                (
                    vec![BpBeneficiary::new(
                        crate::rgb::compat::address_bp_from_pay2vout(
                            pay2vout,
                            invoice.address_network(),
                        ),
                        Sats::from_sats(params.min_amount),
                    )],
                    Some(script),
                )
            }
        };

        // Avoid dust transactions
        let mut inputs = prev_outpoints.clone();
        let mut total_sats = self
            .coins()
            .filter(|x| prev_outpoints.contains(&x.outpoint))
            .map(|x| x.amount)
            .sum();

        let transfer_fee = params.tx.fee_sats;
        let transfer_min = Sats::from_sats(transfer_fee) + self.descriptor().class().dust_limit();
        if total_sats < transfer_min {
            let avaliable_utxos = self.utxos().filter(|x| {
                if prev_outpoints.contains(&x.outpoint) {
                    return false;
                };

                if !matches!(x.status, TxStatus::Mined(_)) {
                    return false;
                }

                // FIX: Skip UTXOs with stale tapret commitments
                // Check if this UTXO's tapret references contracts we no longer have state for
                // This prevents "witness misses contract information" errors when using
                // swap change outputs that reference sold contracts
                if let Ok(tapret_contracts) =
                    stock.contracts_assigning([crate::rgb::compat::outpoint_bp_to_bitcoin(
                        x.outpoint,
                    )])
                {
                    for tapret_contract_id in tapret_contracts {
                        // FIX: Check ALL tapret contracts (including main contract) for actual allocations
                        // If UTXO has tapret commitment to a contract without allocations, it's stale
                        // contracts_assigning() returns contracts based on index/witness references,
                        // but we need to verify actual OUTPUT allocations exist on this UTXO
                        let has_allocations_on_utxo = stock.contract_assignments_for(
                            tapret_contract_id,
                            [crate::rgb::compat::outpoint_bp_to_bitcoin(x.outpoint)],
                        )
                            .map(|assignments| !assignments.is_empty())
                            .unwrap_or(false);

                        if !has_allocations_on_utxo {
                            debug!(format!(
                                "coinselect.skip_stale_tapret utxo={} tapret_contract={} is_main={} reason=no_allocations_on_this_utxo",
                                x.outpoint, tapret_contract_id, tapret_contract_id == contract_id
                            ));
                            return false; // Skip this UTXO - has stale tapret commitment
                        }
                    }
                }

                true
            });

            for utxo in avaliable_utxos {
                total_sats += utxo.value;
                inputs.push(utxo.outpoint);

                if total_sats >= transfer_min {
                    break;
                }
            }
        };

        if total_sats < transfer_min {
            return Err(CompositionError::Construction(
                ConstructionError::NoFundsForFee {
                    input_value: total_sats,
                    output_value: transfer_min,
                    fee: Sats::from_sats(transfer_fee),
                },
            ));
        }

        let (mut psbt, mut meta) = self.construct_psbt(inputs, &beneficiaries, params.tx.into())?;

        let change_script = meta
            .change_vout
            .and_then(|vout| psbt.output(vout.to_usize()))
            .map(|output| output.script.clone());

        match close_method {
            CloseMethod::TapretFirst => {
                let tap_out_script = if let Some(change_script) = change_script.clone() {
                    psbt.set_rgb_tapret_host_on_change();
                    change_script
                } else {
                    match invoice.beneficiary.into_inner() {
                        Beneficiary::WitnessVout(_, Some(ikey)) => {
                            let beneficiary_script = beneficiary_script.unwrap();
                            psbt.outputs_mut()
                                .find(|o| o.script == beneficiary_script)
                                .unwrap()
                                .tap_internal_key =
                                Some(crate::rgb::compat::untweakedpublickey_to_internal_pk(ikey));
                            beneficiary_script
                        }
                        _ => return Err(CompositionError::NoOutputForTapretCommitment),
                    }
                };
                psbt.outputs_mut()
                    .find(|o| o.script.is_p2tr() && o.script == tap_out_script)
                    .map(|o| o.set_tapret_host());
                // TODO: Add descriptor id to the tapret host data
                psbt.sort_outputs_by(|output| !output.is_tapret_host())
                    .expect("PSBT must be modifiable at this stage");
            }
            CloseMethod::OpretFirst => {
                let output = psbt.construct_output_expect(ScriptPubkey::op_return(&[]), Sats::ZERO);
                let _ = output.set_opret_host();
                psbt.sort_outputs_by(|output| !output.is_opret_host())
                    .expect("PSBT must be modifiable at this stage");
            }
        }

        if let Some(ref change_script) = change_script {
            for output in psbt.outputs() {
                if output.script == *change_script {
                    meta.change_vout = Some(output.vout());
                    break;
                }
            }
        }

        let beneficiary_vout = match invoice.beneficiary.into_inner() {
            Beneficiary::WitnessVout(pay2vout, _) => {
                let s = crate::rgb::compat::script_pubkey_bp_from_pay2vout(pay2vout);
                let vout = psbt
                    .outputs()
                    .find(|output| output.script == s)
                    .map(psbt::Output::vout)
                    .expect("PSBT without beneficiary address");
                debug_assert_ne!(Some(vout), meta.change_vout);
                Some(vout)
            }
            Beneficiary::BlindedSeal(_) => None,
        };

        #[allow(clippy::type_complexity)]
        let output_for_assignment =
            |assignment_type: AssignmentType| -> Result<BuilderSeal<GraphSeal>, CompositionError> {
                let vout = meta
                    .change_vout
                    .ok_or(CompositionError::NoExtraOrChange(assignment_type))?;
                let seal = GraphSeal::with_blinded_vout(
                    crate::rgb::compat::vout_bp_to_rgb(vout),
                    rand::random(),
                );
                Ok(BuilderSeal::Revealed(seal))
            };

        let builder_seal = match (invoice.beneficiary.into_inner(), beneficiary_vout) {
            (Beneficiary::BlindedSeal(seal), None) => BuilderSeal::Concealed(seal),
            (Beneficiary::BlindedSeal(_), Some(_)) => {
                return Err(CompositionError::BeneficiaryVout);
            }
            (Beneficiary::WitnessVout(_, _), Some(vout)) => {
                let seal = GraphSeal::with_blinded_vout(
                    crate::rgb::compat::vout_bp_to_rgb(vout),
                    rand::random(),
                );
                BuilderSeal::Revealed(seal)
            }
            (Beneficiary::WitnessVout(_, _), None) => {
                return Err(CompositionError::NoBeneficiaryOutput);
            }
        };

        let mut main_builder = stock
            .transition_builder_raw(contract_id, transition_type)
            .map_err(|e| e.to_string())?;

        let prev_outputs = prev_outputs.into_iter().collect::<HashSet<OutputSeal>>();
        let mut main_inputs = Vec::<OutputSeal>::new();
        let mut sum_inputs = Amount::ZERO;
        let mut data_inputs = vec![];
        for (output, list) in stock
            .contract_assignments_for(contract_id, prev_outputs.iter().copied())
            .map_err(|e| e.to_string())?
        {
            main_inputs.push(output);
            for (opout, state) in list {
                main_builder = main_builder.add_input(opout, state.clone())?;
                if opout.ty != *assignment_type {
                    let seal = output_for_assignment(opout.ty)?;
                    main_builder = main_builder.add_owned_state_raw(opout.ty, seal, state)?;
                } else if let AllocatedState::Amount(value) = state {
                    sum_inputs += value.into();
                } else if let AllocatedState::Data(value) = state {
                    data_inputs.push(value);
                }
            }
        }

        // Add payments to beneficiary and change
        match assignment_state.clone() {
            InvoiceState::Amount(amt) => {
                // Pay beneficiary
                if sum_inputs < amt {
                    return Err(CompositionError::InsufficientState);
                }

                if amt > Amount::ZERO {
                    main_builder =
                        main_builder.add_fungible_state_raw(*assignment_type, builder_seal, amt)?;
                }

                // Pay change
                if sum_inputs > amt {
                    let change_seal = output_for_assignment(*assignment_type)?;
                    main_builder = main_builder.add_fungible_state_raw(
                        *assignment_type,
                        change_seal,
                        sum_inputs - amt,
                    )?;
                }
            }
            InvoiceState::Data(data) => match data {
                NonFungible::FractionedToken(allocation) => {
                    let lookup_state = RevealedData::from(allocation);
                    if !data_inputs.into_iter().any(|x| x == lookup_state) {
                        return Err(CompositionError::InsufficientState);
                    }

                    main_builder =
                        main_builder.add_data_raw(*assignment_type, builder_seal, lookup_state)?;
                }
            },
            InvoiceState::Void => {
                main_builder = main_builder.add_rights_raw(*assignment_type, builder_seal)?;
            }
        }

        // 3. Prepare other transitions
        // Enumerate state
        let mut extra_state =
            HashMap::<ContractId, HashMap<OutputSeal, HashMap<Opout, AllocatedState>>>::new();
        for id in stock
            .contracts_assigning(prev_outputs.iter().copied())
            .map_err(|e| e.to_string())?
        {
            debug!(format!(
                "transfer.extras.checking_contract contract_id={} is_main={} main_contract={}",
                id,
                id == contract_id,
                contract_id
            ));

            // Skip current contract
            if id == contract_id {
                debug!(format!(
                    "transfer.extras.skip_main_contract contract_id={} reason=is_main_contract",
                    id
                ));
                continue;
            }

            // FIX: Check if this contract has ANY allocations on the specific UTXOs being spent
            // This is the ONLY check needed - if the contract has allocations on these UTXOs, include it
            // The previous "global unspent allocations" check was buggy and caused contracts to be
            // included in tapret without actual transitions, leading to "witness misses contract" errors
            let has_allocations_on_inputs = stock
                .contract_assignments_for(id, prev_outputs.iter().copied())
                .map(|assignments| !assignments.is_empty())
                .unwrap_or(false);

            if !has_allocations_on_inputs {
                debug!(format!(
                    "transfer.extras.skip_zero_allocations contract_id={} reason=no_allocations_on_spent_utxos",
                    id
                ));
                continue;
            }

            // Get assignments ONLY on the specific prev_outputs being spent
            let state = stock
                .contract_assignments_for(id, prev_outputs.iter().copied())
                .map_err(|e| e.to_string())?;

            debug!(format!(
                "transfer.extras.include_contract contract_id={} assignments_count={}",
                id,
                state.len()
            ));

            let entry = extra_state.entry(id).or_default();
            for (seal, assigns) in state {
                entry.entry(seal).or_default().extend(assigns);
            }
        }

        // Construct transitions for extra state
        // FIX: Create ONE transition per contract with ALL inputs
        // Previous code created one transition per input, which could result in multiple
        // transitions for the same contract if it had multiple inputs
        let mut extras = Confined::<Vec<_>, 0, { U24 - 1 }>::with_capacity(extra_state.len());
        for (id, seal_map) in extra_state {
            let schema = stock
                .as_stash_provider()
                .contract_schema(id)
                .map_err(|_| BuilderError::Inconsistency(StashInconsistency::ContractAbsent(id)))?;

            // Collect ALL inputs for this contract across all output seals
            let mut all_inputs: Vec<(Opout, AllocatedState)> = Vec::new();
            for (_output, assigns) in seal_map {
                for (opout, state) in assigns {
                    all_inputs.push((opout, state));
                }
            }

            if all_inputs.is_empty() {
                continue;
            }

            // Get transition type from the first input
            let first_opout = all_inputs[0].0;
            let transition_type = schema.default_transition_for_assignment(&first_opout.ty);

            let mut extra_builder = stock
                .transition_builder_raw(id, transition_type)
                .map_err(|e| e.to_string())?;

            // Add ALL inputs to the single transition
            for (opout, state) in &all_inputs {
                extra_builder = extra_builder.add_input(*opout, state.clone())?;
            }

            // Group and sum amounts by assignment type for fungible assets (RGB20)
            // Non-fungible assets (RGB21) are added individually
            let mut amounts_by_type: BTreeMap<AssignmentType, u64> = BTreeMap::new();
            let mut non_fungible_states: Vec<(AssignmentType, AllocatedState)> = Vec::new();

            for (opout, state) in &all_inputs {
                match state {
                    AllocatedState::Amount(revealed_value) => {
                        // Fungible asset - sum the amounts
                        *amounts_by_type.entry(opout.ty).or_insert(0) += revealed_value.as_u64();
                    }
                    AllocatedState::Data(_) => {
                        // Non-fungible asset - keep separate
                        non_fungible_states.push((opout.ty, state.clone()));
                    }
                    AllocatedState::Void => {
                        // Void state - skip
                    }
                }
            }

            // Add summed fungible amounts as outputs
            for (assignment_type, total_amount) in amounts_by_type.iter() {
                debug!(format!(
                    "transfer.extras.sum_fungible contract={} type={} total_amount={}",
                    id, assignment_type, total_amount
                ));
                let seal = output_for_assignment(*assignment_type)?;
                let amount_state = Amount::from(*total_amount);
                let allocated = AllocatedState::Amount(RevealedValue::from(amount_state));
                extra_builder =
                    extra_builder.add_owned_state_raw(*assignment_type, seal, allocated)?;
            }

            // Add non-fungible states individually
            for (assignment_type, state) in non_fungible_states {
                let seal = output_for_assignment(assignment_type)?;
                extra_builder = extra_builder.add_owned_state_raw(assignment_type, seal, state)?;
            }

            if !extra_builder.has_inputs() {
                continue;
            }

            let transition = extra_builder.complete_transition()?;

            debug!(format!(
                "transfer.extras.transition_created contract={} transition_id={} inputs_count={}",
                id,
                transition.id(),
                transition.inputs().len()
            ));

            extras
                .push(transition)
                .map_err(|_| CompositionError::TooManyExtras)?;
        }

        if !main_builder.has_inputs() {
            return Err(CompositionError::InsufficientState);
        }

        let main = main_builder.complete_transition()?;

        debug!(format!(
            "transfer.before_embed main_contract={} main_transition_id={} main_inputs_count={} extras_count={}",
            contract_id,
            main.id(),
            main.inputs().len(),
            extras.len()
        ));

        for (idx, extra) in extras.iter().enumerate() {
            debug!(format!(
                "transfer.before_embed.extra idx={} contract={} transition_id={} inputs_count={}",
                idx,
                extra.contract_id,
                extra.id(),
                extra.inputs().len()
            ));
        }

        let mut batch = Batch { main, extras };
        batch.set_priority(u64::MAX);

        psbt.set_rgb_close_method(close_method);
        psbt.complete_construction();
        psbt.rgb_embed(batch)?;

        // Diagnostic: Check which contracts are registered in PSBT after embed
        match psbt.rgb_contract_ids() {
            Ok(contract_ids) => {
                debug!(format!(
                    "transfer.after_embed.rgb_contract_ids count={} ids=[{}]",
                    contract_ids.len(),
                    contract_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));

                // Check if main contract is in the list
                if !contract_ids.contains(&contract_id) {
                    debug!(format!(
                        "transfer.after_embed.ERROR main_contract={} NOT_FOUND_IN_PSBT",
                        contract_id
                    ));
                }

                // Check consumers for each contract
                for cid in contract_ids {
                    if let Ok(consumers) = psbt.rgb_contract_consumers(cid) {
                        debug!(format!(
                            "transfer.after_embed.consumers contract={} count={} is_main={}",
                            cid,
                            consumers.len(),
                            cid == contract_id
                        ));
                    }
                }
            }
            Err(e) => {
                debug!(format!(
                    "transfer.after_embed.rgb_contract_ids.ERROR error={:?}",
                    e
                ));
            }
        }

        Ok((psbt, meta))
    }
}

struct ContractOutpointsFilter<
    'stock,
    'wallet,
    W: WalletProvider + ?Sized,
    S: StashProvider,
    H: StateProvider,
    P: IndexProvider,
> {
    contract_id: ContractId,
    stock: &'stock Stock<S, H, P>,
    wallet: &'wallet W,
}

impl<W: WalletProvider + ?Sized, S: StashProvider, H: StateProvider, P: IndexProvider>
    AssignmentsFilter for ContractOutpointsFilter<'_, '_, W, S, H, P>
{
    fn should_include(
        &self,
        output: impl Into<rgbstd::Outpoint>,
        id: Option<rgbcore::Txid>,
    ) -> bool {
        let output = output.into();
        if !self.wallet.filter_unspent().should_include(output, id) {
            return false;
        }
        matches!(self.stock.contract_assignments_for(self.contract_id, [output]), Ok(list) if !list.is_empty())
    }
}
