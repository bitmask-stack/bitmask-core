#![allow(clippy::result_large_err)] // SwapErrors is the RGB 0.11 product Result; do not reshape
use crate::bp::models::BtcAccount;
use crate::debug;
use crate::rgb::compat::TentativeWitness;
use crate::rgb::constants::LIB_NAME_BITMASK;
use crate::rgb::data::RGBStock;
use crate::rgb::ext::{PsbtExt, PsbtOrderInExt, ShardErrors};
use crate::rgb::models::{
    AssetKit, AssetType, BidAsset, ContractKit, CounterParty, PublicRgbBid, PublicRgbOffer,
    RgbAccount, RgbBid, RgbOffer, UtxoKit,
};
use crate::rgb::prefetch::BitmaskResolver;
use crate::rgb::transfer::{RgbInvoiceOptions, create_rgb_invoice};
use crate::structs::{
    ChainFee, RgbInvoiceRequest, RgbInvoiceValue, RgbSelection, SecretString, UDAValue,
};
use crate::wallet::data::CrossStoreError;
use amplify::confinement::{Confined, U16};
use amplify::hex::FromHex;
use armor::AsciiArmor;
use autosurgeon::{Hydrate, Reconcile};
use bpstd::psbt::ConstructionError;
use bpstd::{
    Address, NormalIndex, Outpoint, Prevout, Psbt, PsbtVer, Sats, SeqNo, Tx, TxIn, TxOut, Vout,
};
use bpwallet::psbt::PsbtConstructor;
use bpwallet::{Keychain, SighashType, Terminal};
use dbc::Method;
use dbc::Method::TapretFirst;
use nonasync::persistence::CloneNoPersistence;
use psrgbt::{EmbedError, RgbOutExt, RgbPsbtExt};
use rgb::containers::SealWitness;
use rgb::persistence::{IndexReadProvider, StashReadProvider};
use rgb::{CompletionError, TransitionBundle};
use rgbcore::SecretSeal;
use rgbcore::seals::txout::CloseMethod;
use rgbcore::{ChainNet, ContractId, GraphSeal, Layer1, Opout, OutputSeal};
use rgbinvoice::{Beneficiary, RgbInvoice};
use rgbstd::containers::{ConsignmentId, Contract};
use rgbstd::contract::{AllocatedState, BuilderError, ContractError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::str::FromStr;
use strict_encoding::{DefaultBasedStrictDumb, StrictDeserialize, StrictDumb, StrictSerialize};
use strict_types::{StrictDecode, StrictEncode, StrictType};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum OrderSupplError {
    /// there is no contract {0}
    NoContract(String),
    /// there is no contract anchored at this outpoint
    NoContractOutpoint,
    /// Swap can only be carried out with a fixed fee
    NoSupportFee,
    /// Swap can only be with utxo, not address.
    NoUtxo,
    /// wrong counterparty order
    WrongCounterParty,
    /// wrong psbt input. Invalid {0} prevout
    WrongInput(String),
    /// wrong psbt output. Invalid {0} txout
    WrongOutput(String),
    /// wrong change output. invalid {0} terminal
    WrongChange(String),
    /// wrong address output. {0}
    WrongAddress(String),
    /// blank assignment fail. {0}
    WrongAssign(String),
    /// blank transitions fail. {0}
    WrongBlank(String),
    /// rgb batch construction fail. {0}
    WrongBatch(String),
    /// cannot be creating beneficiary. {0}
    WrongBeneficiary(String),
    /// cannot be access rgb_stock history. {0}
    StockError(String),
    Builder(BuilderError),
    /// cannot be constructing psbt. {0}
    Psbt(String),
}

#[derive(
    Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug, Default, Reconcile, Hydrate, Display,
)]
#[serde(rename_all = "camelCase")]
#[display(inner)]
pub enum RgbSwap {
    #[default]
    #[serde(rename = "auction")]
    Auction,
    #[serde(rename = "p2p")]
    P2P,
    #[serde(rename = "hotswap")]
    HotSwap,
    #[serde(rename = "airdrop")]
    Airdrop,
}

#[derive(Clone)]
/// Options used in swap asset operation
pub struct OfferOptions {
    /// Feature-flag to enable sign offer psbt
    pub sign: bool,
    /// Feature-flag to enable send data to proxy server
    pub proxy: bool,
    /// Feature-flag to enable marketplace fee
    pub marketplace_fee: bool,
    /// Define layer1 chain
    pub layer: Layer1,
    /// Define close method
    pub close_method: Method,
    /// Private key(s) for PSBT signing (flag sign=true)
    pub private_keys: Vec<SecretString>,
    /// Optional airdrop fee (strategy=airdrop)
    pub airdrop_fee: Option<u64>,
    /// Optional auctions collection ID (strategy= auction | airdrop)
    pub bundle_id: Option<String>,
}

impl Default for OfferOptions {
    fn default() -> Self {
        Self {
            sign: false,
            proxy: true,
            marketplace_fee: true,
            layer: Layer1::Bitcoin,
            close_method: TapretFirst,
            bundle_id: None,
            private_keys: vec![],
            airdrop_fee: None,
        }
    }
}

impl OfferOptions {
    pub fn p2p(secrets: Vec<SecretString>) -> Self {
        Self {
            sign: true,
            proxy: true,
            marketplace_fee: true,
            layer: Layer1::Bitcoin,
            close_method: TapretFirst,
            bundle_id: None,
            private_keys: secrets,
            airdrop_fee: None,
        }
    }
}

#[derive(Clone)]
/// Options used in swap asset operation
pub struct BidOptions {
    /// Feature-flag to enable sign offer psbt
    pub sign: bool,
    /// Feature-flag to enable send data to proxy server
    pub proxy: bool,
    /// Define layer1 chain
    pub layer: Layer1,
    /// Define close method
    pub close_method: Method,
    /// Private key(s) for PSBT signing (flag sign=true)
    pub private_keys: Vec<SecretString>,
    /// Optional offer entire data (pure p2p swap)
    pub offer: Option<PublicRgbOffer>,
}

impl Default for BidOptions {
    fn default() -> Self {
        Self {
            sign: false,
            proxy: true,
            layer: Layer1::Bitcoin,
            close_method: TapretFirst,
            private_keys: vec![],
            offer: None,
        }
    }
}

impl BidOptions {
    pub fn p2p(secrets: Vec<SecretString>) -> Self {
        Self {
            sign: true,
            proxy: true,
            layer: Layer1::Bitcoin,
            close_method: TapretFirst,
            private_keys: secrets,
            offer: None,
        }
    }

    pub fn debug(secrets: Vec<SecretString>, offer: PublicRgbOffer) -> Self {
        Self {
            sign: true,
            proxy: true,
            layer: Layer1::Bitcoin,
            close_method: TapretFirst,
            private_keys: secrets,
            offer: Some(offer),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, StrictType, StrictEncode, StrictDecode, Debug, Default)]
#[strict_type(lib = LIB_NAME_BITMASK)]
pub struct RgbExtraShards {
    pub extras: Confined<Vec<RgbExtraShard>, 0, { U16 - 1 }>,
}

impl StrictSerialize for RgbExtraShards {}
impl StrictDeserialize for RgbExtraShards {}
impl DefaultBasedStrictDumb for RgbExtraShards {}

#[derive(Clone, Serialize, Deserialize, StrictType, StrictEncode, StrictDecode, Debug)]
#[strict_type(lib = LIB_NAME_BITMASK)]
pub struct RgbExtraShard {
    pub contract_id: ContractId,
    #[serde(with = "crate::rgb::compat::contract_serde")]
    pub contract: Contract,
    pub inputs: Confined<BTreeMap<Opout, AllocatedState>, 0, { U16 - 1 }>,
    pub anchors: Confined<BTreeMap<rgbcore::Txid, AnchorShard>, 0, { U16 - 1 }>,
}

impl StrictSerialize for RgbExtraShard {}
impl StrictDeserialize for RgbExtraShard {}
impl DefaultBasedStrictDumb for RgbExtraShard {}

impl Default for RgbExtraShard {
    fn default() -> Self {
        Self {
            contract_id: ContractId::strict_dumb(),
            contract: Contract::strict_dumb(),
            inputs: Confined::new(),
            anchors: Confined::new(),
        }
    }
}

impl RgbExtraShard {
    fn new(
        contract_id: ContractId,
        contract: Contract,
        inputs: BTreeMap<Opout, AllocatedState>,
        anchors: BTreeMap<rgbcore::Txid, AnchorShard>,
    ) -> Self {
        Self {
            contract_id,
            contract,
            inputs: Confined::from_checked(inputs),
            anchors: Confined::from_checked(anchors),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, StrictType, StrictEncode, StrictDecode, Debug)]
#[strict_type(lib = LIB_NAME_BITMASK)]
pub struct RgbShard {
    pub inputs: Confined<BTreeMap<Opout, AllocatedState>, 0, { U16 - 1 }>,
    pub anchors: Confined<BTreeMap<rgbcore::Txid, AnchorShard>, 0, { U16 - 1 }>,
}

impl StrictSerialize for RgbShard {}
impl StrictDeserialize for RgbShard {}
impl DefaultBasedStrictDumb for RgbShard {}

impl Default for RgbShard {
    fn default() -> Self {
        Self {
            inputs: Confined::new(),
            anchors: Confined::new(),
        }
    }
}

impl RgbShard {
    fn new(
        inputs: BTreeMap<Opout, AllocatedState>,
        anchors: BTreeMap<rgbcore::Txid, AnchorShard>,
    ) -> Self {
        Self {
            inputs: Confined::from_checked(inputs),
            anchors: Confined::from_checked(anchors),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, StrictType, StrictEncode, StrictDecode, Debug)]
#[strict_type(lib = LIB_NAME_BITMASK)]
pub struct AnchorShard {
    pub witness_seal: SealWitness,
    pub bundle: TransitionBundle,
}

impl Default for AnchorShard {
    fn default() -> Self {
        Self {
            witness_seal: SealWitness::strict_dumb(),
            bundle: TransitionBundle::strict_dumb(),
        }
    }
}

impl AnchorShard {
    fn new(witness_seal: SealWitness, bundle: TransitionBundle) -> Self {
        Self {
            witness_seal,
            bundle,
        }
    }
}

// Persisted tapret host selection for seller side
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct HostTerminal {
    pub keychain: Keychain,
    pub index: NormalIndex,
}

impl StrictSerialize for AnchorShard {}
impl StrictDeserialize for AnchorShard {}
impl DefaultBasedStrictDumb for AnchorShard {}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct OrderSuppl {
    pub psbt: String,
    pub shard: Option<String>,
    pub beneficiary: Option<String>,
    pub extra: Option<String>,
    // Persisted seller host terminal (keychain,index) to locate tapret host post-merge
    pub seller_host_terminal: Option<HostTerminal>,
    // Strong host binding: seller's host output scriptPubKey (hex) that MUST be preserved post-merge
    pub seller_host_spk: Option<String>,
}

impl OrderSuppl {
    pub fn l1(psbt: Psbt) -> Self {
        Self {
            psbt: psbt.to_base64_ver(PsbtVer::V0),
            shard: None,
            extra: None,
            beneficiary: None,
            seller_host_terminal: None,
            seller_host_spk: None,
        }
    }

    pub fn rgb(
        inputs: BTreeMap<Opout, AllocatedState>,
        anchors: BTreeMap<rgbcore::Txid, AnchorShard>,
        extra: Vec<RgbExtraShard>,
        psbt: Psbt,
    ) -> Self {
        let strict_shard = RgbShard::new(inputs, anchors)
            .to_strict_serialized::<U16>()
            .unwrap()
            .to_ascii_armored_string();

        let extra = if !extra.is_empty() {
            let extras: Confined<Vec<RgbExtraShard>, 0, { U16 - 1 }> =
                Confined::from_slice_checked(extra.as_slice());
            let extra_shards = RgbExtraShards { extras };
            let strict_shard = extra_shards
                .to_strict_serialized::<U16>()
                .unwrap()
                .to_ascii_armored_string();
            Some(strict_shard)
        } else {
            none!()
        };

        Self {
            psbt: psbt.to_base64_ver(PsbtVer::V0),
            shard: Some(strict_shard),
            extra,
            beneficiary: None,
            seller_host_terminal: None,
            seller_host_spk: None,
        }
    }

    pub fn add_invoice(&mut self, invoice: RgbInvoice) {
        self.beneficiary = Some(invoice.to_string());
    }

    pub fn add_seal(&mut self, seal: SecretSeal) {
        self.beneficiary = Some(seal.to_string());
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SwapSupplement {
    pub tapret: String,
    pub psbt: String,
    pub transfers: BTreeMap<ConsignmentId, String>,
    #[serde(default)]
    pub extras_anchors: BTreeMap<ContractId, BTreeMap<rgbcore::Txid, AnchorShard>>,
    #[serde(default)]
    pub fascia: Option<String>, // Serialized fascia for extras anchor consumption
}

impl RgbOffer {
    pub(crate) fn create_suppl(
        &mut self,
        prev_allocatinos: Vec<Outpoint>,
        btc_account: &mut BtcAccount,
        rgb_account: &mut RgbAccount,
        rgb_stock: &mut RGBStock,
    ) -> Result<OrderSuppl, OrderSupplError> {
        let contract_id = self.contract_id();
        let seqno = SeqNo::from_consensus_u32(0);

        // :: Create PSBT ::
        let mut psbt = Psbt::create(PsbtVer::V2);
        let change_vout = Vout::from_u32(0);

        // Inputs
        let inputs = self.asset().inputs();
        for input in inputs.clone() {
            let UtxoKit {
                outpoint, value, ..
            } = input.clone();
            let prev_out =
                Prevout::new(crate::rgb::compat::outpoint_bitcoin_to_bp(outpoint), value);

            if let Some((_, spk)) = rgb_account
                .wallet
                .utxo(crate::rgb::compat::outpoint_bitcoin_to_bp(input.outpoint))
            {
                let new_psbt_input = psbt
                    .construct_input(
                        prev_out,
                        rgb_account.wallet.descriptor(),
                        input.terminal,
                        spk,
                        seqno,
                    )
                    .map_err(|_| OrderSupplError::WrongInput(format!("{outpoint}#{value}")))?;
                new_psbt_input.sighash_type = Some(SighashType::none_anyone_can_pay())
            }
        }

        // Debug: planned offer inputs and total value
        {
            let planned_inputs_dbg: Vec<String> = inputs
                .iter()
                .map(|x| format!("{}@{}", x.outpoint, x.value))
                .collect();
            let total_sats: Sats = inputs.iter().map(|x| x.value).sum();
            debug!(format!(
                "offer.create_suppl planned_inputs={} total_sats={}",
                planned_inputs_dbg.join(","),
                total_sats
            ));
        }

        // (Change) Output host selection:
        // Prefer a keychain that preserves ownership of "extras".
        // If any extra contracts on these inputs are RGB20, pick RGB20 keychain; otherwise use offered family.
        let input_outs: Vec<_> = inputs.iter().map(|u| u.outpoint).collect();
        let mut prefer_rgb20 = false;
        if let Ok(it) = rgb_stock.contracts_assigning(input_outs.clone()) {
            for id in it {
                if id == contract_id {
                    continue;
                }
                if let Ok(contract) = rgb_stock.export_contract(id) {
                    // Determine family from schema instead of relying on a missing iface field
                    let contract_type = AssetType::from_schema(contract.schema.schema_id());
                    if matches!(contract_type, Some(AssetType::RGB20)) {
                        prefer_rgb20 = true;
                        break;
                    }
                }
            }
        }
        let asset_keychain = if prefer_rgb20 {
            AssetType::RGB20.to_keychain()
        } else {
            self.ty().to_keychain()
        };
        let change_index = rgb_account
            .wallet
            .next_derivation_index(asset_keychain, true);
        let change_terminal = Terminal::new(asset_keychain, change_index);

        // Debug: chosen seller host terminal (possibly adjusted to protect extras)
        debug!(format!(
            "offer.create_suppl host_terminal keychain={:?} index={:?}",
            asset_keychain, change_index
        ));

        let change_amount: Sats = inputs.clone().into_iter().map(|x| x.value).sum();

        psbt.construct_change(
            rgb_account.wallet.descriptor(),
            change_terminal,
            change_amount,
        )
        .map_err(|_| OrderSupplError::WrongChange(format!("{change_terminal}")))?;

        // (Offer) Output
        let bitcoin_keychain = AssetType::Bitcoin.to_keychain();
        let offer_index = btc_account.wallet.last_derivation_index(bitcoin_keychain);
        let offer_terminal = Terminal::new(bitcoin_keychain, offer_index);
        let offer_address = btc_account.wallet.next_address(bitcoin_keychain, false);

        if let CounterParty::Sats(offer_value) = self.counter_party() {
            let offer_value = Sats::from(*offer_value);
            psbt.construct_output(offer_address.script_pubkey(), offer_value)
                .map_err(|_| OrderSupplError::WrongChange(format!("{offer_terminal}")))?;
        }

        // :: Contract Shard ::
        let prev_outputs = psbt
            .to_unsigned_tx()
            .inputs
            .iter()
            .map(|txin| txin.prev_output)
            .collect::<HashSet<Outpoint>>();

        let mut asset_available_amt = 0;

        // Prepare main transition
        let mut allocation = bmap! {};
        let mut contract_inputs = BTreeMap::new();

        for (output_seal, opout_state_map) in rgb_stock
            .contract_assignments_for(
                contract_id,
                prev_outputs
                    .iter()
                    .filter(|op| {
                        inputs
                            .iter()
                            .any(|x| crate::rgb::compat::outpoint_bitcoin_to_bp(x.outpoint) == **op)
                    })
                    .copied()
                    .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
            )
            .map_err(|_| OrderSupplError::NoContractOutpoint)?
        {
            for (opout, state) in opout_state_map {
                if let AllocatedState::Amount(amt) = &state {
                    asset_available_amt += amt.as_u64();
                }

                allocation.insert(output_seal.to_outpoint(), contract_id);
                contract_inputs.insert(opout, state.clone());
            }
        }

        // Debug: prev_outputs, allocation and inputs summary
        {
            let prev_dbg: Vec<String> = prev_outputs.iter().map(|o| o.to_string()).collect();
            debug!(format!(
                "offer.create_suppl prev_outputs_count={} prev_outputs={}",
                prev_dbg.len(),
                prev_dbg.join(",")
            ));
            debug!(format!(
                "offer.create_suppl asset_available_amt={} allocation_inputs={} contract_inputs={}",
                asset_available_amt,
                allocation.len(),
                contract_inputs.len()
            ));
        }

        // Debug: PSBT outputs derivations before selecting tapret host
        {
            let outs_dbg: Vec<String> = psbt
                .outputs()
                .enumerate()
                .map(|(i, o)| {
                    if let Some(der) = o.terminal_derivation() {
                        format!("{}:{:?}/{:?}", i, der.keychain, der.index)
                    } else {
                        format!("{}:no-derivation", i)
                    }
                })
                .collect();
            debug!(format!(
                "offer.create_suppl outputs_derivations_before_tapret={}",
                outs_dbg.join(",")
            ));
        }

        // Select tapret host explicitly on the seller's chosen change terminal
        let mut found_tapret = false;
        let mut host_vout_idx: Option<usize> = None;
        for (i, out) in psbt.outputs_mut().enumerate() {
            if let Some(der) = out.terminal_derivation()
                && der.keychain == asset_keychain
                && der.index == change_index
            {
                let _ = out.set_tapret_host();
                found_tapret = true;
                host_vout_idx = Some(i);
                break;
            }
        }
        if !found_tapret {
            return Err(OrderSupplError::NoContractOutpoint);
        }
        // Compute seller host scriptPubKey binding (hex) for preservation checks during merge
        let mut host_spk_hex: Option<String> = None;
        if let Some(vout) = host_vout_idx {
            let spk = psbt.to_unsigned_tx().outputs[vout].script_pubkey.clone();
            let hex = spk
                .as_slice()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            host_spk_hex = Some(hex);
        }

        for (output, contract_id) in &allocation {
            if let Some(input) = psbt.inputs_mut().find(|x| {
                x.previous_outpoint == crate::rgb::compat::outpoint_bitcoin_to_bp(*output)
            }) {
                let amount = self.amount().to_value();
                let allocated = if asset_available_amt > 0 && amount < asset_available_amt {
                    asset_available_amt - amount
                } else {
                    0
                };

                let _ = input.set_rgb_offer_in(*contract_id, allocated);
            }
        }

        // Retrieve all bundles
        let stash = rgb_stock.as_stash_provider().clone_no_persistence();
        let index = rgb_stock.as_index_provider().clone_no_persistence();

        #[allow(clippy::expect_used)]
        let mut current_opouts = index
            .public_opouts(contract_id)
            .expect("Infallible index read");

        #[allow(clippy::expect_used)]
        let outputs = allocation.clone().into_keys();
        current_opouts.extend(
            index
                .opouts_by_outputs(contract_id, outputs)
                .expect("Infallible index read"),
        );

        current_opouts.extend(
            index
                .opouts_by_outputs(
                    contract_id,
                    prev_allocatinos
                        .iter()
                        .copied()
                        .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
                )
                .expect("Infallible index read"),
        );

        let mut anchors = bmap! {};
        let mut previous_opouts = vec![];
        for opout in current_opouts {
            if previous_opouts.contains(&opout) {
                continue;
            }
            if let Ok(bundle_id) = index.bundle_id_for_op(opout.op) {
                if let Ok(bundle) = stash.bundle(bundle_id) {
                    let transitions = bundle
                        .known_transitions
                        .iter()
                        .filter(|known| known.opid == opout.op)
                        .map(|known| known.transition.clone());

                    transitions.for_each(|transition| {
                        transition
                            .assignments
                            .iter()
                            .find(|(ty, _)| *ty == &opout.ty)
                            .map(|(_, assignments)| assignments)
                            .iter()
                            .for_each(|x| {
                                let mut seals_reveled = vec![];
                                let count = x.len_u16();
                                for index in 0..count {
                                    if let Ok(Some(reveal)) = x.revealed_seal_at(index) {
                                        seals_reveled.push(reveal);
                                    }
                                }

                                #[allow(clippy::expect_used)]
                                let (witness_ids, _) =
                                    index.bundle_info(bundle_id).expect("Infallible index read");

                                for witness_id in witness_ids {
                                    #[allow(clippy::expect_used)]
                                    let witness_seal = stash
                                        .witness(witness_id)
                                        .expect("Infallible stash provider");

                                    anchors.insert(
                                        witness_seal.witness_id(),
                                        AnchorShard::new(witness_seal.clone(), bundle.clone()),
                                    );
                                }
                            });
                    });
                }
                previous_opouts.push(opout);
            }
        }

        // Prepare other transition
        let mut extra_state =
            HashMap::<ContractId, HashMap<OutputSeal, HashMap<Opout, AllocatedState>>>::new();

        let another_contracts = rgb_stock
            .contracts_assigning(
                prev_outputs
                    .iter()
                    .filter(|op| {
                        inputs
                            .iter()
                            .any(|x| crate::rgb::compat::outpoint_bitcoin_to_bp(x.outpoint) == **op)
                    })
                    .copied()
                    .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
            )
            .map_err(|_| OrderSupplError::NoContractOutpoint)?;
        {
            // Diagnostic: enumerate contracts assigning to the same prevouts (excluding offered one)
            if let Ok(it) = rgb_stock.contracts_assigning(
                prev_outputs
                    .iter()
                    .filter(|op| {
                        inputs
                            .iter()
                            .any(|x| crate::rgb::compat::outpoint_bitcoin_to_bp(x.outpoint) == **op)
                    })
                    .copied()
                    .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
            ) {
                let ids: Vec<String> = it.map(|id| id.to_string()).collect();
                debug!(format!(
                    "offer.create_suppl another_contracts_on_prevouts={} [{}]",
                    ids.len(),
                    ids.join(",")
                ));
            }
        }

        for id in another_contracts {
            // Skip current contract
            if id == contract_id {
                continue;
            }

            let state = rgb_stock
                .contract_assignments_for(
                    id,
                    prev_outputs
                        .iter()
                        .copied()
                        .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
                )
                .map_err(|e| OrderSupplError::WrongBlank(e.to_string()))?;

            // FIX: Skip contracts with no allocations on the UTXOs being spent
            // contracts_assigning() returns contracts based on index/witness references,
            // but they may not have actual OUTPUT allocations on these specific UTXOs
            if state.is_empty() {
                debug!(format!(
                    "offer.extras.skip_empty contract_id={} reason=no_allocations_on_spent_utxos",
                    id
                ));
                continue;
            }

            let entry = extra_state.entry(id).or_default();
            for (seal, assigns) in state {
                entry.entry(seal).or_default().extend(assigns);
            }
        }

        let mut extra_contracts = vec![];
        for (contract_id, contract_states) in extra_state {
            let mut states: BTreeMap<Opout, AllocatedState> = BTreeMap::new();
            for (output_seal, contract_state) in contract_states {
                if let Some(input) = psbt.inputs_mut().find(|x| {
                    x.previous_outpoint
                        == crate::rgb::compat::outpoint_bitcoin_to_bp(output_seal.to_outpoint())
                }) {
                    let mut allocated = 0;
                    for (opout, state) in contract_state.clone() {
                        match state {
                            AllocatedState::Void => {}
                            AllocatedState::Amount(amount) => {
                                allocated += amount.as_u64();
                            }
                            AllocatedState::Data(_) => {
                                allocated = 1;
                            }
                        }

                        states.insert(opout, state);
                    }

                    let _ = input.set_rgb_offer_in(contract_id, allocated);
                }
            }

            let contract_exported = rgb_stock
                .export_contract(contract_id)
                .map_err(|_| OrderSupplError::NoContract(contract_id.to_string()))?;

            // Collect anchors by iterating through the states (opouts) that are part of this swap
            let mut anchors: BTreeMap<rgbcore::Txid, AnchorShard> = BTreeMap::new();
            let stash = rgb_stock.as_stash_provider().clone_no_persistence();
            let index = rgb_stock.as_index_provider().clone_no_persistence();

            debug!(format!(
                "offer.extra_anchor.start contract_id={} states_count={}",
                contract_id,
                states.len()
            ));

            for opout in states.keys() {
                // Get the bundle that created this opout
                if let Ok(bundle_id) = index.bundle_id_for_op(opout.op)
                    && let Ok(bundle) = stash.bundle(bundle_id)
                    && let Ok((witness_ids, _)) = index.bundle_info(bundle_id)
                {
                    for witness_id in witness_ids {
                        if let Ok(witness_seal) = stash.witness(witness_id) {
                            anchors.insert(
                                witness_seal.witness_id(),
                                AnchorShard::new(witness_seal.clone(), bundle.clone()),
                            );
                            debug!(format!(
                                "offer.extra_anchor.collected contract_id={} witness_id={}",
                                contract_id,
                                witness_seal.witness_id()
                            ));
                        }
                    }
                }
            }

            debug!(format!(
                "offer.extra_anchor.finish contract_id={} anchors_count={}",
                contract_id,
                anchors.len()
            ));

            let rgb_shard = RgbExtraShard::new(contract_id, contract_exported, states, anchors);
            extra_contracts.push(rgb_shard);
        }
        // Debug: list extra contracts discovered for seller
        {
            let ids: Vec<String> = extra_contracts
                .iter()
                .map(|s| s.contract_id.to_string())
                .collect();
            debug!(format!(
                "offer.create_suppl extra_contract_ids={} [{}]",
                ids.len(),
                ids.join(",")
            ));
        }

        // Note: Early pending_host registration for historical witness outpoints removed.
        // Historical outpoints are already known to wallet via indexing.
        // Swap outputs are registered during verification (rgb.rs:647-685).

        let seal = GraphSeal::new_random_vout(crate::rgb::compat::vout_bp_to_rgb(change_vout));
        rgb_stock
            .store_secret_seal(seal)
            .map_err(|_| OrderSupplError::WrongCounterParty)?;

        let mut suppl = OrderSuppl::rgb(contract_inputs, anchors, extra_contracts, psbt);
        // Persist seller host terminal so runtime can save tapret metadata against the correct derivation
        suppl.seller_host_terminal = Some(HostTerminal {
            keychain: asset_keychain,
            index: change_index,
        });
        // Persist strong host binding (scriptPubKey)
        suppl.seller_host_spk = host_spk_hex;
        suppl.add_seal(seal.to_secret_seal());

        Ok(suppl)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct NewBidParams {
    chain_fee: ChainFee,
}

impl NewBidParams {
    pub fn with(chain_fee: ChainFee) -> Self {
        Self { chain_fee }
    }
}

impl RgbBid {
    pub(crate) fn create_suppl(
        &mut self,
        params: NewBidParams,
        btc_account: &mut BtcAccount,
        rgb_account: &mut RgbAccount,
        rgb_stock: &mut RGBStock,
    ) -> Result<OrderSuppl, OrderSupplError> {
        let btc_wallet = btc_account.wallet.clone_no_persistence();
        let rgb_wallet = rgb_account.wallet.clone_no_persistence();
        let seqno = SeqNo::from_consensus_u32(0);

        // ::: Create PSBT :::
        let mut psbt = Psbt::create(PsbtVer::V2);

        // Inputs
        let inputs = self.inputs();

        // Debug planned inputs
        let planned_sum: Sats = inputs.iter().map(|x| x.value).sum();
        let planned_dbg: Vec<String> = inputs
            .iter()
            .map(|x| format!("{}@{}", x.outpoint, x.value))
            .collect();
        debug!(format!(
            "bid.create_suppl planned_inputs={} planned_sum={}",
            planned_dbg.join(","),
            planned_sum
        ));

        // Track successfully inserted inputs and any misses
        let mut inserted_sum: Sats = Sats::ZERO;
        let mut inserted_set: HashSet<Outpoint> = HashSet::new();
        let mut inserted_dbg: Vec<String> = vec![];
        let mut missing_inputs: Vec<String> = vec![];

        for input in &inputs {
            let UtxoKit {
                outpoint, value, ..
            } = input.clone();
            let prev_out =
                Prevout::new(crate::rgb::compat::outpoint_bitcoin_to_bp(outpoint), value);

            if let Some((_, spk)) = rgb_account
                .wallet
                .utxo(crate::rgb::compat::outpoint_bitcoin_to_bp(input.outpoint))
            {
                let new_psbt_input = psbt
                    .construct_input(
                        prev_out,
                        rgb_wallet.descriptor(),
                        input.terminal,
                        spk,
                        seqno,
                    )
                    .map_err(|_| OrderSupplError::WrongInput(format!("{outpoint}#{value}")))?;
                new_psbt_input.sighash_type = Some(SighashType::none_anyone_can_pay());
                inserted_sum += value;
                inserted_set.insert(crate::rgb::compat::outpoint_bitcoin_to_bp(outpoint));
                inserted_dbg.push(format!("rgb:{}@{}", outpoint, value));
            } else if let Some((_, spk)) = btc_account
                .wallet
                .utxo(crate::rgb::compat::outpoint_bitcoin_to_bp(input.outpoint))
            {
                let new_psbt_input = psbt
                    .construct_input(
                        prev_out,
                        btc_wallet.descriptor(),
                        input.terminal,
                        spk,
                        seqno,
                    )
                    .map_err(|_| OrderSupplError::WrongInput(format!("{outpoint}#{value}")))?;
                new_psbt_input.sighash_type = Some(SighashType::none_anyone_can_pay());
                inserted_sum += value;
                inserted_set.insert(crate::rgb::compat::outpoint_bitcoin_to_bp(outpoint));
                inserted_dbg.push(format!("btc:{}@{}", outpoint, value));
            } else {
                // Neither wallet recognized the input outpoint
                missing_inputs.push(format!("{}", outpoint));
            }
        }

        debug!(format!(
            "bid.create_suppl inserted_inputs_initial={} inserted_sum_initial={} missing_inputs={}",
            inserted_dbg.join(","),
            inserted_sum,
            missing_inputs.join(",")
        ));
        {
            // Inspect overlaps: which inserted inputs are also RGB-anchored
            let inserted_vec: Vec<Outpoint> = inserted_set.iter().copied().collect();
            if !inserted_vec.is_empty()
                && let Ok(overlap_contracts) = rgb_stock.contracts_assigning(
                    inserted_vec
                        .iter()
                        .copied()
                        .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
                )
            {
                let ids: Vec<String> = overlap_contracts.map(|id| id.to_string()).collect();
                debug!(format!(
                    "bid.create_suppl overlap_contracts_on_inserted_inputs={}",
                    ids.join(",")
                ));
            }
        }

        // Invoice (Process Pay)
        let input_utxos: Vec<_> = self
            .inputs()
            .iter()
            .map(|x| crate::rgb::compat::outpoint_bitcoin_to_bp(x.outpoint))
            .collect();
        let next_utxo = rgb_account
            .wallet
            .coinselect(Sats::ZERO, |utxo| {
                crate::rgb::compat::rgb_keychain_contains(utxo.terminal.keychain)
                    && !input_utxos.contains(&utxo.outpoint)
            })
            .next();

        let seal_selection = if next_utxo.is_some() {
            &RgbSelection::NextUTXO
        } else {
            &RgbSelection::NextAddress
        };

        let invoice_info = if let CounterParty::Contract(contract_swap) = self.counter_party() {
            let ContractKit { info, .. } = contract_swap.kit.clone();
            let invoice_amount = if info.contract_type().clone().is_uda() {
                RgbInvoiceValue::UDA(UDAValue::default())
            } else {
                RgbInvoiceValue::Value(contract_swap.amount.to_string())
            };

            let invoice_req = RgbInvoiceRequest {
                contract_id: Some(info.contract_id().to_string()),
                amount: invoice_amount,
                seal: seal_selection.clone(),
                expire_at: None,
            };

            let options = RgbInvoiceOptions::default();
            create_rgb_invoice(invoice_req, options, rgb_stock, rgb_account)
                .map_err(|x| OrderSupplError::WrongBeneficiary(x.to_string()))?
        } else {
            return Err(OrderSupplError::WrongCounterParty);
        };

        let NewBidParams { chain_fee } = params;
        let fee = match chain_fee {
            ChainFee::Value(fee) => Sats::from_sats(fee),
            _ => return Err(OrderSupplError::NoSupportFee),
        };

        // Ensure we have enough inserted inputs to cover required minimum: fee + optional offer price
        let required_output = if self.is_sats() {
            self.l1_amount()
        } else {
            Sats::ZERO
        };
        let required_min = required_output + fee;
        if inserted_sum < required_min {
            // Top up from BTC wallet across both keychains with mined UTXOs not already included
            let bitcoin_keychains = [
                AssetType::Bitcoin.to_keychain(),
                AssetType::Bitcoin.to_alt_keychain(),
            ];
            let mut cand: Vec<_> = btc_wallet
                .utxos()
                .filter(|u| {
                    matches!(u.status, bpwallet::TxStatus::Mined(_))
                        && bitcoin_keychains.contains(&u.terminal.keychain)
                        && !inserted_set.contains(&u.outpoint)
                        && !inputs.iter().any(|x| {
                            crate::rgb::compat::outpoint_bitcoin_to_bp(x.outpoint) == u.outpoint
                        })
                })
                .collect();
            // Prefer larger UTXOs first
            cand.sort_by_key(|a| std::cmp::Reverse(a.value));

            let mut topup_dbg: Vec<String> = vec![];
            for u in cand {
                if inserted_sum >= required_min {
                    break;
                }
                // Retrieve full (WalletUtxo, ScriptPubkey) to get spk and terminal for construct_input
                if let Ok((wu, spk)) = btc_account.wallet.outpoint_by(u.outpoint) {
                    let prev_out = Prevout::new(wu.outpoint, wu.value);
                    let new_psbt_input = psbt
                        .construct_input(prev_out, btc_wallet.descriptor(), wu.terminal, spk, seqno)
                        .map_err(|_| {
                            OrderSupplError::WrongInput(format!("{}#{}", wu.outpoint, wu.value))
                        })?;
                    new_psbt_input.sighash_type = Some(SighashType::none_anyone_can_pay());
                    inserted_set.insert(wu.outpoint);
                    inserted_sum += wu.value;
                    inserted_dbg.push(format!("topup-btc:{}@{}", wu.outpoint, wu.value));
                    topup_dbg.push(format!("{}@{}", wu.outpoint, wu.value));
                }
            }

            debug!(format!(
                "bid.create_suppl topup_selected={} inserted_sum_after_topup={} required_min={}",
                topup_dbg.join(","),
                inserted_sum,
                required_min
            ));
        }

        // Use only actually inserted inputs for funding math
        let sats: Sats = inserted_sum;
        let mut sats_remaining = sats.checked_sub(fee).ok_or(OrderSupplError::Psbt(
            ConstructionError::NoFundsForFee {
                input_value: sats,
                output_value: sats + fee,
                fee,
            }
            .to_string(),
        ))?;

        if self.is_sats() {
            let bitcoin_keychain = AssetType::Bitcoin.to_keychain();
            let change_index = btc_wallet.last_derivation_index(bitcoin_keychain);
            let change_terminal = Terminal::new(bitcoin_keychain, change_index);

            let offer_price = self.l1_amount();
            sats_remaining =
                sats_remaining
                    .checked_sub(offer_price)
                    .ok_or(OrderSupplError::Psbt(
                        ConstructionError::OutputExceedsInputs {
                            input_value: sats_remaining,
                            output_value: offer_price,
                        }
                        .to_string(),
                    ))?;

            if seal_selection.is_address() && sats_remaining.is_non_zero() {
                let address = Address::from_str(&invoice_info.seal).expect("invalid secret_seal");
                psbt.construct_output(address.script_pubkey(), sats_remaining)
                    .map_err(|_| OrderSupplError::WrongOutput(format!("{address}")))?;
            } else if sats_remaining.is_non_zero() {
                psbt.construct_change(btc_wallet.descriptor(), change_terminal, sats_remaining)
                    .map_err(|_| OrderSupplError::WrongChange(format!("{change_terminal}")))?;
            } else {
                return Err(OrderSupplError::NoContractOutpoint);
            }
        } else {
            let rgb_keychain = AssetType::RGB20.to_keychain();
            let change_index = btc_wallet.last_derivation_index(rgb_keychain);
            let change_terminal = Terminal::new(rgb_keychain, change_index);
            if seal_selection.is_address() && sats_remaining.is_non_zero() {
                let address = Address::from_str(&invoice_info.seal).expect("invalid secret_seal");
                psbt.construct_output(address.script_pubkey(), sats_remaining)
                    .map_err(|_| OrderSupplError::WrongOutput(format!("{address}")))?;
            } else if sats_remaining.is_non_zero() {
                // Fix: BTC change must use the BTC wallet descriptor, not the RGB wallet descriptor
                psbt.construct_change(btc_wallet.descriptor(), change_terminal, sats_remaining)
                    .map_err(|_| OrderSupplError::WrongChange(format!("{change_terminal}")))?;
            } else {
                return Err(OrderSupplError::NoContractOutpoint);
            }
        }

        // ::: Create Suppl :::
        let mut suppl = match self.asset() {
            BidAsset::L1(_) => OrderSuppl::l1(psbt),
            BidAsset::Contract(contract_swap) => {
                let contract_id = contract_swap.contract_id();

                let prev_outputs = psbt
                    .to_unsigned_tx()
                    .inputs
                    .iter()
                    .map(|txin| txin.prev_output)
                    .collect::<HashSet<Outpoint>>();

                let mut asset_available_amt = 0;
                let mut contract_inputs = BTreeMap::new();

                let mut allocation = bmap! {};

                for (output_seal, opout_state_map) in rgb_stock
                    .contract_assignments_for(
                        contract_id,
                        prev_outputs
                            .iter()
                            .filter(|op| {
                                inputs.iter().any(|x| {
                                    crate::rgb::compat::outpoint_bitcoin_to_bp(x.outpoint) == **op
                                })
                            })
                            .copied()
                            .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
                    )
                    .unwrap()
                {
                    for (opout, state) in opout_state_map {
                        if let AllocatedState::Amount(amt) = &state {
                            asset_available_amt += amt.as_u64();
                        }

                        allocation.insert(output_seal.to_outpoint(), contract_id);
                        contract_inputs.insert(opout, state.clone());
                    }
                }

                let (vout_index, _) = psbt
                    .to_unsigned_tx()
                    .outputs
                    .iter()
                    .enumerate()
                    .find(|(_, o)| o.script_pubkey.is_p2tr())
                    .expect("psbt should have an taproot output");

                let (_, tapret_output) = psbt
                    .outputs_mut()
                    .enumerate()
                    .find(|(i, _)| i == &vout_index)
                    .unwrap();
                let _ = tapret_output.set_tapret_host();

                for (output, contract_id) in &allocation {
                    if let Some(input) = psbt.inputs_mut().find(|x| {
                        x.previous_outpoint == crate::rgb::compat::outpoint_bitcoin_to_bp(*output)
                    }) {
                        let amount = self.amount().to_value();
                        let allocated = if asset_available_amt > 0 && amount < asset_available_amt {
                            asset_available_amt - amount
                        } else {
                            0
                        };

                        input.set_rgb_bid_in(*contract_id, allocated).unwrap();
                    }
                }

                // Retrieve all bundles
                let stash = rgb_stock.as_stash_provider().clone_no_persistence();
                let index = rgb_stock.as_index_provider().clone_no_persistence();

                #[allow(clippy::expect_used)]
                let mut current_opouts = index
                    .public_opouts(contract_id)
                    .expect("Infallible index read");

                #[allow(clippy::expect_used)]
                let outputs = allocation.into_keys();
                current_opouts.extend(
                    index
                        .opouts_by_outputs(contract_id, outputs)
                        .expect("Infallible index read"),
                );

                let mut anchors = bmap! {};
                let mut previous_opouts = vec![];
                for opout in current_opouts {
                    if previous_opouts.contains(&opout) {
                        continue;
                    }
                    if let Ok(bundle_id) = index.bundle_id_for_op(opout.op) {
                        if let Ok(bundle) = stash.bundle(bundle_id) {
                            let transitions = bundle
                                .known_transitions
                                .iter()
                                .filter(|known| known.opid == opout.op)
                                .map(|known| known.transition.clone());

                            transitions.for_each(|transition| {
                                transition
                                    .assignments
                                    .iter()
                                    .find(|(ty, _)| *ty == &opout.ty)
                                    .map(|(_, assignments)| assignments)
                                    .iter()
                                    .for_each(|x| {
                                        let mut seals_reveled = vec![];
                                        let count = x.len_u16();
                                        for index in 0..count {
                                            if let Ok(Some(reveal)) = x.revealed_seal_at(index) {
                                                seals_reveled.push(reveal);
                                            }
                                        }

                                        #[allow(clippy::expect_used)]
                                        let (witness_ids, _) = index
                                            .bundle_info(bundle_id)
                                            .expect("Infallible index read");

                                        for witness_id in witness_ids {
                                            #[allow(clippy::expect_used)]
                                            let witness_seal = stash
                                                .witness(witness_id)
                                                .expect("Infallible stash provider");

                                            anchors.insert(
                                                witness_seal.witness_id(),
                                                AnchorShard::new(
                                                    witness_seal.clone(),
                                                    bundle.clone(),
                                                ),
                                            );
                                        }
                                    });
                            });
                        }
                        previous_opouts.push(opout);
                    }
                }

                // Prepare other transition
                let mut extra_state = HashMap::<
                    ContractId,
                    HashMap<OutputSeal, HashMap<Opout, AllocatedState>>,
                >::new();

                for id in rgb_stock
                    .contracts_assigning(
                        prev_outputs
                            .iter()
                            .filter(|op| {
                                inputs.iter().any(|x| {
                                    crate::rgb::compat::outpoint_bitcoin_to_bp(x.outpoint) == **op
                                })
                            })
                            .copied()
                            .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
                    )
                    .unwrap()
                {
                    // Skip current contract
                    if id == contract_id {
                        continue;
                    }

                    let state = rgb_stock
                        .contract_assignments_for(
                            id,
                            prev_outputs
                                .iter()
                                .copied()
                                .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
                        )
                        .map_err(|e| OrderSupplError::WrongBlank(e.to_string()))?;

                    // FIX: Skip contracts with no allocations on the UTXOs being spent
                    // contracts_assigning() returns contracts based on index/witness references,
                    // but they may not have actual OUTPUT allocations on these specific UTXOs
                    if state.is_empty() {
                        debug!(format!(
                            "bid.extras.skip_empty contract_id={} reason=no_allocations_on_spent_utxos",
                            id
                        ));
                        continue;
                    }

                    let entry = extra_state.entry(id).or_default();
                    for (seal, assigns) in state {
                        entry.entry(seal).or_default().extend(assigns);
                    }
                }

                let mut extra_contracts = vec![];
                for (contract_id, contract_states) in extra_state {
                    let mut states: BTreeMap<Opout, AllocatedState> = BTreeMap::new();
                    for (output_seal, contract_state) in contract_states {
                        if let Some(input) = psbt.inputs_mut().find(|x| {
                            x.previous_outpoint
                                == crate::rgb::compat::outpoint_bitcoin_to_bp(
                                    output_seal.to_outpoint(),
                                )
                        }) {
                            let mut allocated = 0;
                            for (opout, state) in contract_state.clone() {
                                match state {
                                    AllocatedState::Void => {}
                                    AllocatedState::Amount(amount) => {
                                        allocated += amount.as_u64();
                                    }
                                    AllocatedState::Data(_) => {
                                        allocated = 1;
                                    }
                                }
                                states.insert(opout, state);
                            }

                            // input.set_rgb_offer_in(contract_id, allocated).unwrap();
                            input.set_rgb_bid_in(contract_id, allocated).unwrap();
                        }
                    }

                    let contract_exported = rgb_stock
                        .export_contract(contract_id)
                        .expect("no contract exported");

                    // Collect anchors by iterating through the states (opouts) that are part of this swap
                    let mut extra_anchors: BTreeMap<rgbcore::Txid, AnchorShard> = BTreeMap::new();
                    let stash = rgb_stock.as_stash_provider().clone_no_persistence();
                    let index = rgb_stock.as_index_provider().clone_no_persistence();

                    debug!(format!(
                        "bid.extra_anchor.start contract_id={} states_count={}",
                        contract_id,
                        states.len()
                    ));

                    for opout in states.keys() {
                        // Get the bundle that created this opout
                        if let Ok(bundle_id) = index.bundle_id_for_op(opout.op)
                            && let Ok(bundle) = stash.bundle(bundle_id)
                            && let Ok((witness_ids, _)) = index.bundle_info(bundle_id)
                        {
                            for witness_id in witness_ids {
                                if let Ok(witness_seal) = stash.witness(witness_id) {
                                    extra_anchors.insert(
                                        witness_seal.witness_id(),
                                        AnchorShard::new(witness_seal.clone(), bundle.clone()),
                                    );
                                    debug!(format!(
                                        "bid.extra_anchor.collected contract_id={} witness_id={}",
                                        contract_id,
                                        witness_seal.witness_id()
                                    ));
                                }
                            }
                        }
                    }

                    debug!(format!(
                        "bid.extra_anchor.finish contract_id={} anchors_count={}",
                        contract_id,
                        extra_anchors.len()
                    ));

                    let rgb_shard =
                        RgbExtraShard::new(contract_id, contract_exported, states, extra_anchors);
                    extra_contracts.push(rgb_shard);
                }

                // Note: Early pending_host registration for historical witness outpoints removed.
                // Historical outpoints are already known to wallet via indexing.
                // Swap outputs are registered during verification (rgb.rs:647-685).

                OrderSuppl::rgb(contract_inputs, anchors, extra_contracts, psbt)
            }
        };

        suppl.add_invoice(invoice_info.invoice);
        self.add_suppl(suppl.clone());

        Ok(suppl)
    }
}

#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum SwapErrors {
    /// I/O or connectivity error. {0}
    IO(CrossStoreError),
    /// store retrieve error. {0}
    Store(String),
    /// explorer sync error. {0}
    Explorer(String),
    /// rgb contract iface error. {0}
    ContractIfaceError(String, String),
    /// rgb contract error. {0}
    ContractError(#[from] ContractError),
    /// strict encoding error. {0}
    StrictEncodingError(#[from] strict_encoding::InvalidRString),
    /// invalid partial psbt.
    WrongPsbt(String),
    /// invalid offer
    WrongOffer,
    /// invalid bid
    WrongBid,
    /// invalid invoice. {0}
    InvalidInvoice(String),
    /// no commit output
    NoOutput,
    /// no tapret output
    NoTapret,
    /// rgb transfer error. {0}
    Transfer(String),
    /// rgb pay error. {0}
    Pay(String),
    /// rgb contract shard error. {0}
    Shard(ShardErrors),
    /// rgb embed error. {0}
    Embed(EmbedError),
    /// rgb completion error
    CompletionError(#[from] CompletionError),
    /// rgb commit error. {0}
    CommitError(#[from] psrgbt::CommitError),
}

impl PublicRgbOffer {
    pub(crate) fn swap(
        self,
        chain_net: ChainNet,
        public_bid: PublicRgbBid,
        rgb_stock: &mut RGBStock,
        rgb_resolver: &mut BitmaskResolver,
    ) -> Result<SwapSupplement, SwapErrors> {
        // 1. Merge PSBT
        let PublicRgbOffer {
            order_suppl: offer_suppl,
            ..
        } = self;

        let PublicRgbBid {
            suppl: bid_suppl,
            asset: bid_asset,
            amount: bid_amount,
            ..
        } = public_bid.clone();

        let offer_suppl = offer_suppl.unwrap();
        let bid_suppl = bid_suppl.unwrap();

        let seller_psbt = offer_suppl.psbt.clone();
        let seller_shard = offer_suppl.shard;
        let seller_extra_shard = offer_suppl.extra;

        let buyer_psbt = bid_suppl.psbt.clone();
        let buyer_invoice = bid_suppl.beneficiary.clone().unwrap_or_default();
        let buyer_shard = bid_suppl.shard;
        let buyer_extra_shard = bid_suppl.extra;

        let rgb_invoice = RgbInvoice::from_str(&buyer_invoice)
            .map_err(|_| SwapErrors::InvalidInvoice(buyer_invoice.to_string()))?;

        let offer_psbt = Psbt::from_base64(&seller_psbt).map_err(|_| SwapErrors::WrongOffer)?;
        let bid_psbt = Psbt::from_base64(&buyer_psbt).map_err(|_| SwapErrors::WrongBid)?;

        let mut offer_inputs: Vec<_> = offer_psbt.inputs().collect();
        let mut offer_outputs: Vec<_> = offer_psbt.outputs().collect();

        let mut bids_inputs: Vec<_> = bid_psbt.inputs().collect();
        let mut bids_outputs: Vec<_> = bid_psbt.outputs().collect();

        let mut swap_inputs = if bids_inputs.is_empty() {
            vec![offer_inputs.remove(0)]
        } else {
            vec![bids_inputs.remove(0), offer_inputs.remove(0)]
        };

        let mut swap_outputs = if offer_outputs.len() <= 1 {
            vec![offer_outputs.remove(0)]
        } else {
            vec![offer_outputs.remove(0), offer_outputs.remove(0)]
        };

        if bids_outputs.len() > 1 {
            bids_outputs.remove(0); // Discard redundancy
        }

        offer_inputs.extend(bids_inputs);
        for input in offer_inputs {
            swap_inputs.push(input);
        }

        offer_outputs.extend(bids_outputs);
        for output in offer_outputs {
            swap_outputs.push(output);
        }

        // 2. Create PSBT Swap. Dummy ins/outs are cloned over from the merged
        // offer/bid PSBT pieces; do not use rgb-strict-encoding StrictDumb on
        // bp-std types (those implement strict_encoding 2.8.2).
        let dummy_in = TxIn {
            prev_output: Outpoint::new(bpstd::Txid::from([0u8; 32]), Vout::from_u32(0)),
            sig_script: default!(),
            sequence: SeqNo::from_consensus_u32(0),
            witness: none!(),
        };
        let dummy_out = TxOut::new(bpstd::ScriptPubkey::op_return(&[]), Sats::ZERO);
        let mut swap_tx_tmpl = Tx {
            version: default!(),
            inputs: empty!(),
            outputs: empty!(),
            lock_time: none!(),
        };
        swap_tx_tmpl.inputs.clear();
        swap_tx_tmpl.outputs.clear();

        for _ in 0..swap_inputs.len() {
            swap_tx_tmpl
                .inputs
                .push(dummy_in.clone())
                .expect("invalid input size");
        }
        for _ in 0..swap_outputs.len() {
            swap_tx_tmpl
                .outputs
                .push(dummy_out.clone())
                .expect("invalid output size");
        }

        let mut swap_psbt = Psbt::from_tx(swap_tx_tmpl);
        for (index, input) in swap_psbt.inputs_mut().enumerate() {
            input.clone_from(swap_inputs[index]);
        }
        for (index, output) in swap_psbt.outputs_mut().enumerate() {
            output.clone_from(swap_outputs[index]);
        }

        swap_psbt.proprietary.extend(offer_psbt.proprietary.clone());
        swap_psbt.proprietary.extend(bid_psbt.proprietary.clone());

        // 4. Generate RGB Commit
        // Prefer the seller's strong host binding (scriptPubKey), then persisted host terminal; fall back to first P2TR output.
        let mut host_set = false;

        // A) Enforce scriptPubKey host binding if present
        if let Some(ref spk_hex) = offer_suppl.seller_host_spk {
            if let Ok(spk_bytes) = Vec::<u8>::from_hex(spk_hex) {
                if let Some((vout_index, _)) = swap_psbt
                    .to_unsigned_tx()
                    .outputs
                    .iter()
                    .enumerate()
                    .find(|(_, o)| o.script_pubkey.as_slice() == spk_bytes.as_slice())
                {
                    let _ = swap_psbt
                        .output_mut(vout_index)
                        .ok_or(SwapErrors::NoOutput)?
                        .set_tapret_host();
                    host_set = true;
                } else {
                    return Err(SwapErrors::WrongPsbt(
                        "Host binding violated (seller host script not found in merged PSBT)"
                            .to_string(),
                    ));
                }
            } else {
                return Err(SwapErrors::WrongPsbt(
                    "Host binding decode error".to_string(),
                ));
            }
        }

        // B) If no binding, use persisted host terminal (keychain/index)
        if !host_set && let Some(host) = offer_suppl.seller_host_terminal.clone() {
            for out in swap_psbt.outputs_mut() {
                if let Some(der) = out.terminal_derivation()
                    && der.keychain == host.keychain
                    && der.index == host.index
                {
                    let _ = out.set_tapret_host();
                    host_set = true;
                    break;
                }
            }
        }
        if !host_set {
            return Err(SwapErrors::WrongPsbt(
                "No seller host found in merged PSBT (binding and persisted terminal missing)"
                    .to_string(),
            ));
        }

        swap_psbt.set_rgb_close_method(CloseMethod::TapretFirst);

        let seller_extra_shards = if let Some(serialized) = seller_extra_shard {
            let serialized = Vec::<u8>::from_ascii_armored_str(&serialized)
                .map_err(|err| SwapErrors::Shard(ShardErrors::Parse(err.to_string())))?;
            let confined = Confined::from_slice_checked(serialized.as_slice());

            let rgb_shards = RgbExtraShards::from_strict_serialized::<U16>(confined)
                .map_err(|err| SwapErrors::Shard(ShardErrors::Parse(err.to_string())))?;
            rgb_shards.extras.to_vec()
        } else {
            vec![]
        };

        let buyer_extra_shards = if let Some(serialized) = buyer_extra_shard {
            let serialized = Vec::<u8>::from_ascii_armored_str(&serialized)
                .map_err(|err| SwapErrors::Shard(ShardErrors::Parse(err.to_string())))?;
            let confined = Confined::from_slice_checked(serialized.as_slice());

            let rgb_shards = RgbExtraShards::from_strict_serialized::<U16>(confined)
                .map_err(|err| SwapErrors::Shard(ShardErrors::Parse(err.to_string())))?;
            rgb_shards.extras.to_vec()
        } else {
            vec![]
        };

        let bid_seal = if let Some(serialized) = seller_shard {
            let serialized = Vec::<u8>::from_ascii_armored_str(&serialized)
                .map_err(|err| SwapErrors::Shard(ShardErrors::Parse(err.to_string())))?;
            let confined = Confined::from_slice_checked(serialized.as_slice());

            let rgb_shard = RgbShard::from_strict_serialized::<U16>(confined)
                .map_err(|err| SwapErrors::Shard(ShardErrors::Parse(err.to_string())))?;

            swap_psbt
                .pay_buyer(rgb_invoice.clone(), rgb_shard, rgb_stock)
                .map_err(|x| SwapErrors::Pay(x.to_string()))?
        } else {
            match rgb_invoice.beneficiary.into_inner() {
                Beneficiary::BlindedSeal(seal) => seal,
                Beneficiary::WitnessVout(addr, _) => {
                    let s = crate::rgb::compat::script_pubkey_bp_from_pay2vout(addr);
                    let vout = swap_psbt
                        .outputs()
                        .position(|output| output.script == s)
                        .ok_or(CompletionError::NoBeneficiaryOutput)
                        .expect("rgb swap");
                    let vout = Vout::from_u32(vout as u32);

                    let seal = GraphSeal::new_random_vout(crate::rgb::compat::vout_bp_to_rgb(vout));
                    rgb_stock.store_secret_seal(seal).expect("rgb swap");
                    seal.to_secret_seal()
                }
            }
        };

        let offer_seal = SecretSeal::from_str(&offer_suppl.beneficiary.unwrap_or_default())
            .expect("invalid secret seal");
        if let Some(serialized) = buyer_shard {
            let serialized = Vec::<u8>::from_ascii_armored_str(&serialized)
                .map_err(|err| SwapErrors::Shard(ShardErrors::Parse(err.to_string())))?;
            let confined = Confined::from_slice_checked(serialized.as_slice());

            let rgb_shard = RgbShard::from_strict_serialized::<U16>(confined)
                .map_err(|err| SwapErrors::Shard(ShardErrors::Parse(err.to_string())))?;

            if let AssetKit::Contract(bid_kit) = bid_asset.clone() {
                swap_psbt
                    .pay_seller(offer_seal, bid_kit, bid_amount, rgb_shard, rgb_stock)
                    .map_err(|x| SwapErrors::Pay(x.to_string()))?;
            }
        }

        swap_psbt
            .add_extra_seller(
                chain_net,
                seller_extra_shards.clone(),
                rgb_stock,
                rgb_resolver,
            )
            .expect("rgb swap");
        swap_psbt
            .add_extra_buyer(
                chain_net,
                buyer_extra_shards.clone(),
                rgb_stock,
                rgb_resolver,
            )
            .expect("rgb swap");

        swap_psbt.complete_construction();

        // 5. Generate Transfer
        let contract_id = rgb_invoice.contract.ok_or(CompletionError::NoContract)?;

        // Diagnostic: Check which contracts are registered in PSBT before rgb_commit
        match swap_psbt.rgb_contract_ids() {
            Ok(contract_ids) => {
                debug!(format!(
                    "swap.before_commit.rgb_contract_ids count={} ids=[{}]",
                    contract_ids.len(),
                    contract_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));

                if !contract_ids.contains(&contract_id) {
                    debug!(format!(
                        "swap.before_commit.ERROR main_contract={} NOT_FOUND_IN_PSBT",
                        contract_id
                    ));
                }

                for cid in contract_ids {
                    if let Ok(consumers) = swap_psbt.rgb_contract_consumers(cid) {
                        debug!(format!(
                            "swap.before_commit.consumers contract={} count={} is_main={}",
                            cid,
                            consumers.len(),
                            cid == contract_id
                        ));
                    }
                }
            }
            Err(e) => {
                debug!(format!(
                    "swap.before_commit.rgb_contract_ids.ERROR error={:?}",
                    e
                ));
            }
        }

        let fascia = swap_psbt.rgb_commit()?;

        // Diagnostic: Check what contracts are actually in the committed fascia
        debug!(format!(
            "swap.after_commit.fascia bundles_count={} witness_txid={}",
            fascia.bundles().len(),
            fascia.seal_witness().witness_id()
        ));

        let bundle_contract_ids: Vec<String> =
            fascia.bundles().keys().map(|id| id.to_string()).collect();

        debug!(format!(
            "swap.after_commit.fascia.contracts contracts=[{}]",
            bundle_contract_ids.join(", ")
        ));

        // Check if the main contract and extras are all present in the fascia
        if !fascia.bundles().contains_key(&contract_id) {
            debug!(format!(
                "swap.after_commit.fascia.ERROR main_contract={} NOT_IN_FASCIA",
                contract_id
            ));
        }

        for extra_shard in seller_extra_shards.iter().chain(buyer_extra_shards.iter()) {
            let extra_contract_id = extra_shard.contract_id;
            if !fascia.bundles().contains_key(&extra_contract_id) {
                debug!(format!(
                    "swap.after_commit.fascia.ERROR extra_contract={} NOT_IN_FASCIA",
                    extra_contract_id
                ));
            } else {
                debug!(format!(
                    "swap.after_commit.fascia.extra_present contract={}",
                    extra_contract_id
                ));
            }
        }

        // CRITICAL DIAGNOSTIC: Check which bundle IDs are actually in the tapret merkle block
        let known_bundle_ids: Vec<String> = fascia
            .seal_witness()
            .known_bundle_ids()
            .map(|id| id.to_string())
            .collect();

        debug!(format!(
            "swap.after_commit.merkle_block.known_bundles count={} bundles=[{}]",
            known_bundle_ids.len(),
            known_bundle_ids.join(", ")
        ));

        // Map bundle IDs back to contract IDs to see which contracts are in tapret commitment
        for (cid, bundle) in fascia.bundles().iter() {
            let bundle_id = bundle.bundle_id();
            let in_merkle_block = fascia
                .seal_witness()
                .known_bundle_ids()
                .any(|bid| bid == bundle_id);
            debug!(format!(
                "swap.after_commit.contract_in_merkle contract={} bundle={} in_tapret={}",
                cid, bundle_id, in_merkle_block
            ));
        }

        let witness_txid = swap_psbt.txid();
        let beneficiary1 = vec![bid_seal];
        let beneficiary2 = vec![];

        let mut transfers = bmap! {};
        rgb_stock
            .consume_fascia(fascia.clone(), TentativeWitness::from_bp(witness_txid))
            .expect("invalid fascia");

        let rgb_transfer = rgb_stock
            .transfer(
                contract_id,
                beneficiary2.clone(),
                beneficiary1.clone(),
                [],
                Some(crate::rgb::compat::txid_bp_to_bitcoin(witness_txid)),
            )
            .map_err(|e| SwapErrors::Pay(e.to_string()))?;

        transfers.insert(rgb_transfer.consignment_id(), format!("{rgb_transfer}"));
        if let AssetKit::Contract(bid_kit) = bid_asset {
            let beneficiary1 = vec![offer_seal];
            let beneficiary2 = vec![];

            let rgb_transfer = rgb_stock
                .transfer(
                    *bid_kit.info.contract_id(),
                    beneficiary2,
                    beneficiary1,
                    [],
                    Some(crate::rgb::compat::txid_bp_to_bitcoin(witness_txid)),
                )
                .map_err(|e| SwapErrors::Pay(e.to_string()))?;
            transfers.insert(rgb_transfer.consignment_id(), format!("{rgb_transfer}"));
        }

        // 6. Collect anchors for extras from the serialized RgbExtraShard anchors field
        let mut extras_anchors: BTreeMap<ContractId, BTreeMap<rgbcore::Txid, AnchorShard>> =
            BTreeMap::new();

        // Collect seller extras anchors
        // Note: Anchors were collected during offer creation and serialized in RgbExtraShard
        for extra_shard in seller_extra_shards {
            let contract_id = extra_shard.contract_id;
            let anchors = extra_shard.anchors.to_unconfined();

            crate::debug!(format!(
                "swap.extras_anchors.seller contract_id={} anchors_count={}",
                contract_id,
                anchors.len()
            ));

            if !anchors.is_empty() {
                extras_anchors.insert(contract_id, anchors.clone());
                for witness_id in anchors.keys() {
                    crate::debug!(format!(
                        "swap.extras_anchors.collected seller contract_id={} witness_id={}",
                        contract_id, witness_id
                    ));
                }
            }
        }

        // Collect buyer extras anchors
        // Note: Anchors were collected during bid creation and serialized in RgbExtraShard
        for extra_shard in buyer_extra_shards {
            let contract_id = extra_shard.contract_id;
            let anchors = extra_shard.anchors.to_unconfined();

            crate::debug!(format!(
                "swap.extras_anchors.buyer contract_id={} anchors_count={}",
                contract_id,
                anchors.len()
            ));

            if !anchors.is_empty() {
                extras_anchors.insert(contract_id, anchors.clone());
                for witness_id in anchors.keys() {
                    crate::debug!(format!(
                        "swap.extras_anchors.collected buyer contract_id={} witness_id={}",
                        contract_id, witness_id
                    ));
                }
            }
        }

        // 7. Generate Rgb Swap
        let tapret = swap_psbt.export_commit().expect("invalid tapret");

        // Serialize the fascia for use during verification
        let fascia_serialized = fascia
            .to_strict_serialized::<U16>()
            .ok()
            .map(|bytes| bytes.to_ascii_armored_string());

        let swap_suppl = SwapSupplement {
            tapret: tapret.to_string(),
            psbt: swap_psbt.to_base64_ver(PsbtVer::V0),
            transfers,
            extras_anchors,
            fascia: fascia_serialized,
        };

        // 8. Sync Provider

        Ok(swap_suppl)
    }
}
