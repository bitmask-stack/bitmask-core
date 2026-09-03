#![allow(clippy::unwrap_used)]
use crate::debug;
use crate::rgb::contract::extract_transfer;
use crate::rgb::data::RgbAccountInfo;
use crate::rgb::ext::derive_fungible_amount_from_consignment;
use crate::rgb::swap::{OrderSuppl, SwapSupplement};
use crate::structs::{ContractResponse, OrderValue, UDAValue};
use crate::wallet::data::BPWallet;
use amplify::{Array, Bytes32};
use armor::AsciiArmor;
use autosurgeon::{Hydrate, Reconcile};
use bpstd::{NormalIndex, Sats};
use bpwallet::{Keychain, Psbt, Terminal, TxStatus, Txid};
use dbc::tapret::TapretCommitment;
use nonasync::persistence::CloneNoPersistence;
use rand::prelude::StdRng;
use rand::{RngCore, SeedableRng};
use rgb::Outpoint;
use rgbcore::SecretSeal;
use rgbcore::{ChainNet, ContractId, OutputSeal, SchemaId};
use rgbinvoice::{Beneficiary, InvoiceState, NonFungible, RgbInvoice};
use rgbstd::containers::{ConsignmentExt, ConsignmentId, Transfer};
use rgbstd::contract::FilterIncludeAll;
use schemata::{CFA_SCHEMA_ID, IFA_SCHEMA_ID, NIA_SCHEMA_ID, PFA_SCHEMA_ID, UDA_SCHEMA_ID};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::str::FromStr;
use strict_encoding::StrictDumb;

#[derive(Clone, PartialEq, Debug, Display, Error, From, Eq)]
#[display(doc_comments)]
pub enum AssetParseError {
    /// The asset type {0} is not supported
    NoSupport(String),
}

#[derive(Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Display, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[display(inner)]
#[repr(u8)]
pub enum AssetType {
    #[serde(rename = "bitcoin")]
    Bitcoin,
    #[serde(rename = "rgb20")]
    RGB20,
    #[serde(rename = "rgb21")]
    RGB21,
    #[serde(rename = "cfa")]
    CFA,
    #[serde(rename = "ifa")]
    IFA,
    #[serde(rename = "pfa")]
    PFA,
}

impl AssetType {
    pub fn is_uda(&self) -> bool {
        matches!(&self, AssetType::RGB21)
    }

    pub fn is_fungible(&self) -> bool {
        matches!(
            &self,
            AssetType::RGB20 | AssetType::CFA | AssetType::IFA | AssetType::PFA
        )
    }

    pub fn to_keychain(&self) -> Keychain {
        match self {
            AssetType::Bitcoin => Keychain::with(0),
            _ => Keychain::with(10),
        }
    }

    pub fn to_alt_keychain(&self) -> Keychain {
        match self {
            AssetType::Bitcoin => Keychain::with(1),
            _ => Keychain::with(9),
        }
    }

    pub fn to_interface_name(&self) -> String {
        match self {
            AssetType::Bitcoin => "Bitcoin".to_string(),
            AssetType::RGB20 => "RGB20Fixed".to_string(),
            AssetType::RGB21 => "RGB21Unique".to_string(),
            AssetType::CFA => "CFA".to_string(),
            AssetType::IFA => "IFA".to_string(),
            AssetType::PFA => "PFA".to_string(),
        }
    }

    pub fn from_schema(schema_id: SchemaId) -> Option<AssetType> {
        if schema_id == UDA_SCHEMA_ID {
            Some(AssetType::RGB21)
        } else if schema_id == NIA_SCHEMA_ID {
            Some(AssetType::RGB20)
        } else if schema_id == CFA_SCHEMA_ID {
            Some(AssetType::CFA)
        } else if schema_id == IFA_SCHEMA_ID {
            Some(AssetType::IFA)
        } else if schema_id == PFA_SCHEMA_ID {
            Some(AssetType::PFA)
        } else {
            None
        }
    }
}

impl FromStr for AssetType {
    type Err = AssetParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rgb" => Ok(AssetType::RGB21),
            "bitcoin" | "btc" => Ok(AssetType::Bitcoin),
            "rgb20" | "rgb20fixed" | "rgb20replaceable" => Ok(AssetType::RGB20),
            "rgb21" | "rgb21unique" => Ok(AssetType::RGB21),
            "cfa" | "rgb25" => Ok(AssetType::CFA),
            "ifa" => Ok(AssetType::IFA),
            "pfa" => Ok(AssetType::PFA),
            other => Err(AssetParseError::NoSupport(other.to_string())),
        }
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone)]
pub enum ContractValue {
    Value(String),
    UDA(UDAValue),
}

impl ContractValue {
    pub fn value(amount: String) -> Self {
        Self::Value(amount)
    }

    pub fn uda(allocation: UDAValue) -> Self {
        Self::UDA(allocation)
    }
    pub fn is_uda(&self) -> bool {
        matches!(self, Self::UDA(_))
    }

    pub fn is_fungible(&self) -> bool {
        matches!(self, Self::Value(_))
    }
}

#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Reconcile,
    Hydrate,
    Debug,
    Default,
)]
pub struct ContractAmount {
    pub int: u64,
    pub fract: u64,
    pub precision: u8,
}

impl ContractAmount {
    /// Define a new contract value.
    ///
    /// An int ([`u64`]) and fract ([`u64`]), combine with
    /// precision ([`u8`]), load a contract value, compabitle
    /// with rgb contract value.
    ///
    /// Remeber: All contract amounts are represents in [`u64`].
    /// The [`ContractAmount`] abstract the calculation.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// use bitmask_core::rgb::models::ContractAmount;
    /// let amount = ContractAmount::new(2, 50, 2);
    ///
    /// assert_eq!(amount.int, 2);
    /// assert_eq!(amount.fract, 50);
    /// assert_eq!(amount.clone().to_value(), 250);
    /// assert_eq!(amount.to_string(), "2.50");
    /// ```
    pub fn new(int: u64, fract: u64, precision: u8) -> Self {
        ContractAmount {
            int,
            fract,
            precision,
        }
    }

    /// Get a contract value.
    ///
    /// A value ([`u64`]) combine with precision ([`u8`]), load
    /// a contract value, compabitle with rgb contract value.
    ///
    /// Remeber: All contract amounts are represents in [`u64`].
    /// The [`ContractAmount`] abstract the calculation.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// use bitmask_core::rgb::models::ContractAmount;
    /// let amount = ContractAmount::with(250, 2);
    ///
    /// assert_eq!(amount.int, 2);
    /// assert_eq!(amount.fract, 50);
    /// assert_eq!(amount.clone().to_value(), 250);
    /// assert_eq!(amount.to_string(), "2.50");
    /// ```
    pub fn with(value: u64, precision: u8) -> Self {
        if value == 0 {
            return ContractAmount::new(0, 0, precision);
        }

        if precision == 0 {
            return ContractAmount::new(value, 0, precision);
        }

        let number = value.to_string();
        let num_len = number.len();

        match num_len.cmp(&precision.into()) {
            Ordering::Greater => {
                let prec = precision as usize;
                let int: u64 = number[0..num_len - prec]
                    .parse()
                    .expect("invalid int number");
                let fract: u64 = number[num_len - prec..num_len]
                    .parse()
                    .expect("invalid fract number");
                ContractAmount::new(int, fract, precision)
            }
            Ordering::Less | Ordering::Equal => ContractAmount::new(0, value, precision),
        }
    }

    /// Convert a decimal string representation of the
    /// number in Contract Amount.
    ///
    /// A value ([`String`]) return a contract value,
    /// compabitle with rgb contract value.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// use bitmask_core::rgb::models::ContractAmount;
    /// let amount = ContractAmount::convert("1.000".to_string());
    ///
    /// assert_eq!(amount.int, 1);
    /// assert_eq!(amount.fract, 0);
    /// assert_eq!(amount.clone().to_value(), 1000);
    /// assert_eq!(amount.to_string(), "1.000");
    ///
    /// let amount = ContractAmount::convert("1.100".to_string());
    /// ```
    pub fn convert(value: String) -> Self {
        let mut fract = 0;
        let mut precision = 0;

        let int = if value.contains('.') {
            let parts = value.split('.');
            let collection: Vec<&str> = parts.collect();

            fract = collection[1].parse().unwrap();
            precision = collection[1].len() as u8;

            collection[0].parse().unwrap()
        } else {
            value.parse().unwrap()
        };

        ContractAmount {
            int,
            fract,
            precision,
        }
    }

    /// Parse a number string representation to
    /// Contract Amount.
    ///
    /// A value ([`String`]) return a contract value,
    /// compabitle with rgb contract value.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// // Define the initial value
    /// use bitmask_core::rgb::models::ContractAmount;
    /// let amount = ContractAmount::parse("1".to_string(), 2);
    ///
    /// assert_eq!(amount.int, 1);
    /// assert_eq!(amount.fract, 0);
    /// assert_eq!(amount.clone().to_value(), 100);
    /// assert_eq!(amount.to_string(), "1.00");
    ///
    /// ```
    pub fn parse(value: String, precision: u8) -> Self {
        // Robust, non-panicking parser:
        // - Trims whitespace
        // - Ignores thousands separators and underscores
        // - Accepts optional '.' fractional part
        // - Pads/truncates fractional digits to 'precision'
        let s = value.trim().replace(['_', ','], "");
        if s.is_empty() {
            return ContractAmount::new(0, 0, precision);
        }

        if precision == 0 {
            let number = s.parse::<u64>().unwrap_or(0);
            return ContractAmount::new(number, 0, 0);
        }

        if let Some(dot_idx) = s.find('.') {
            let int_part = &s[..dot_idx];
            let fract_part = &s[dot_idx + 1..];

            // Parse integer part; default to 0 if invalid
            let int = int_part.parse::<u64>().unwrap_or(0);

            // Normalize fractional to the requested precision
            let mut fract = fract_part
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>();

            let prec = precision as usize;
            if fract.len() < prec {
                fract.push_str(&"0".repeat(prec - fract.len()));
            } else if fract.len() > prec {
                fract.truncate(prec);
            }

            let fract_u = fract.parse::<u64>().unwrap_or(0);

            if int == 0 && fract_u == 0 {
                return ContractAmount::new(0, 0, precision);
            }
            ContractAmount::new(int, fract_u, precision)
        } else {
            let number = s.parse::<u64>().unwrap_or(0);
            if number == 0 {
                return ContractAmount::new(0, 0, precision);
            }
            ContractAmount::new(number, 0, precision)
        }
    }

    pub fn to_value(self) -> u64 {
        let pow = 10_u64.pow(self.precision as u32);
        self.int * pow + self.fract
    }
}

impl Display for ContractAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let precision = self.precision as usize;
        if precision > 0 {
            write!(f, "{}.{:0precision$}", self.int, self.fract)
        } else {
            write!(f, "{}", self.int)
        }
    }
}

#[derive(
    Getters, Clone, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Display,
)]
#[display(doc_comments)]
pub struct ContractInfo {
    contract_id: ContractId,
    contract_type: AssetType,
    name: String,
    ticker: String,
    precision: u8,
}

impl Default for ContractInfo {
    fn default() -> Self {
        Self {
            contract_id: ContractId::strict_dumb(),
            contract_type: AssetType::RGB21,
            precision: u8::MIN,
            name: String::new(),
            ticker: String::new(),
        }
    }
}

impl ContractInfo {
    pub fn with(contract: ContractResponse) -> Self {
        let ContractResponse {
            name,
            ticker,
            iface,
            precision,
            contract_id,
            ..
        } = contract.clone();
        let contract_type = AssetType::from_str(&iface).expect("invalid contract type");
        let contract_id = ContractId::from_str(&contract_id).expect("invalid contract id");

        Self {
            name,
            ticker,
            precision,
            contract_type,
            contract_id,
        }
    }
}

#[derive(Debug)]
pub struct RgbInvoiceItem {
    pub invoice: RgbInvoice,
    pub seal: String,
    pub payed: bool,
}

#[derive(Debug)]
pub struct RgbAccount {
    chain_net: ChainNet,
    contract_kits: Vec<ContractInfo>,
    hidden_contracts: Vec<ContractId>,

    bids: Vec<RgbBid>,
    offers: Vec<RgbOffer>,
    transfers: Vec<RgbTransfer>,

    pub commits: BTreeMap<Terminal, TapretCommitment>,
    pub pending_hosts: BTreeMap<String, Terminal>,
    pub lookahead_hints: BTreeMap<Keychain, NormalIndex>,
    pub wallet: BPWallet,
}

impl CloneNoPersistence for RgbAccount {
    fn clone_no_persistence(&self) -> Self {
        let RgbAccount { wallet, .. } = self;
        Self {
            bids: self.bids.clone(),
            offers: self.offers.clone(),
            transfers: self.transfers.clone(),
            contract_kits: self.contract_kits.clone(),
            hidden_contracts: self.hidden_contracts.clone(),
            commits: self.commits.clone(),
            pending_hosts: self.pending_hosts.clone(),
            lookahead_hints: self.lookahead_hints.clone(),
            chain_net: self.chain_net,
            wallet: wallet.clone_no_persistence(),
        }
    }
}

impl RgbAccount {
    pub fn new(wallet: BPWallet, chain_net: ChainNet) -> Self {
        Self {
            wallet,
            chain_net,
            bids: Vec::new(),
            offers: Vec::new(),
            transfers: Vec::new(),
            contract_kits: Vec::new(),
            hidden_contracts: Vec::new(),
            pending_hosts: BTreeMap::new(),
            lookahead_hints: BTreeMap::new(),
            commits: BTreeMap::new(),
        }
    }

    pub fn with(wallet: BPWallet, info: RgbAccountInfo) -> Self {
        Self {
            wallet,
            chain_net: info.chain_net,
            bids: info.bids,
            offers: info.offers,
            transfers: info.transfers,
            contract_kits: info.contract_kits,
            hidden_contracts: info.hidden_contracts,
            pending_hosts: info.pending_hosts,
            lookahead_hints: info.lookahead_hints,
            commits: info.commits,
        }
    }

    pub fn chain_net(&self) -> ChainNet {
        self.chain_net
    }

    pub fn info(&self) -> RgbAccountInfo {
        let Self {
            contract_kits,
            hidden_contracts,
            transfers,
            offers,
            bids,
            chain_net,
            ..
        } = self.clone_no_persistence();

        RgbAccountInfo {
            persistence: None,
            chain_net,
            hidden_contracts,
            contract_kits,
            transfers,
            offers,
            bids,
            pending_hosts: self.pending_hosts.clone(),
            lookahead_hints: self.lookahead_hints.clone(),
            commits: self.commits.clone(),
        }
    }

    pub fn contract_by(&self, contract_id: ContractId) -> Option<ContractInfo> {
        self.contract_kits
            .iter()
            .find(|info| info.contract_id == contract_id)
            .cloned()
    }

    pub fn _seals_by(&self, filter: impl AccountFilter) -> Vec<String> {
        self.clone_no_persistence()
            .transfers
            .into_iter()
            .filter(|x| {
                if let Direction::In(inc) = &x.direction {
                    let outpoint = Outpoint::from_str(&inc.utxo).expect("invalid utxo");
                    let output_seal = OutputSeal::with(outpoint.txid, outpoint.vout);
                    filter.should_include(
                        Some(output_seal.to_outpoint()),
                        None,
                        None,
                        Some(x.status),
                    )
                } else {
                    false
                }
            })
            .map(|x| match x.direction {
                Direction::In(inc) => inc.invoice.to_string(),
                _ => "".to_string(),
            })
            .filter(|x| !x.is_empty())
            .collect()
    }

    pub fn transfers_by(&self, filter: impl AccountFilter) -> Vec<RgbTransfer> {
        self.clone_no_persistence()
            .transfers
            .into_iter()
            .filter(|x| {
                filter.should_include(None, Some(x.contract_id), x.transfer_id(), Some(x.status))
            })
            .collect()
    }

    pub fn invoices_by(&self, filter: impl AccountFilter) -> Vec<RgbInvoiceItem> {
        let transfers: Vec<_> = self
            .clone_no_persistence()
            .transfers
            .into_iter()
            .filter(|x| match x.clone().direction {
                Direction::In(inc) => {
                    if inc.utxo.is_empty() {
                        return false;
                    }

                    let outpoint = Outpoint::from_str(&inc.utxo).expect("invalid utxo");
                    let output_seal = OutputSeal::with(outpoint.txid, outpoint.vout);
                    filter.should_include(
                        Some(output_seal.to_outpoint()),
                        Some(x.contract_id),
                        None,
                        Some(x.status),
                    )
                }
                _ => false,
            })
            .collect();

        let mut invoice = vec![];
        for transfer in transfers {
            if let Direction::In(inc) = &transfer.direction {
                let rgb_invoice = RgbInvoice::from_str(&inc.invoice).expect("invalid invoice");
                let seal = if let Beneficiary::BlindedSeal(secret_seal) =
                    rgb_invoice.beneficiary.into_inner()
                {
                    secret_seal.to_string()
                } else {
                    String::new()
                };

                let item = RgbInvoiceItem {
                    invoice: rgb_invoice,
                    payed: transfer.is_payed(),
                    seal,
                };
                invoice.push(item)
            }
        }
        invoice
    }

    pub fn offers_by(&self, filter: impl OfferFilter) -> Vec<RgbOffer> {
        self.clone_no_persistence()
            .offers
            .into_iter()
            .filter(|x| filter.should_include(Some(x.id.to_string()), Some(x.status)))
            .collect()
    }

    pub fn bids_by(&self, filter: impl BidFilter) -> Vec<RgbBid> {
        self.clone_no_persistence()
            .bids
            .into_iter()
            .filter(|x| {
                filter.should_include(
                    Some(x.id.to_string()),
                    Some(x.offer_id.to_string()),
                    Some(x.status),
                )
            })
            .collect()
    }

    pub fn add_tapret(&mut self, commit: TapretCommitment, terminal: Terminal) {
        self.commits.insert(terminal, commit);
    }

    pub fn add_contract(&mut self, rgb_info: ContractInfo) -> Self {
        if !self.contract_kits.contains(&rgb_info.clone()) {
            self.contract_kits.push(rgb_info);
            debug!(format!("add new contract {:#?}", self.contract_kits));
        }
        self.clone_no_persistence()
    }

    pub fn add_invoice(&mut self, rgb_invoice: RgbInvoiceInfo) -> Self {
        let rgb_transfer = RgbTransfer::new_in(rgb_invoice);
        self.transfers.push(rgb_transfer);
        debug!(format!("new transfer in {:#?}", self.transfers));
        self.clone_no_persistence()
    }
    pub fn remove_invoice(&mut self, rgb_invoice: RgbInvoice) -> Self {
        let transfer = self
            .clone_no_persistence()
            .transfers
            .into_iter()
            .position(|x| x.is_in() && x.is_open() && x.has_invoice(rgb_invoice.clone()));

        if let Some(index) = transfer {
            self.transfers.remove(index);
        }
        self.clone_no_persistence()
    }
    pub fn add_transfer(&mut self, rgb_info: RgbTransferInfo) -> Self {
        let rgb_transfer = RgbTransfer::new_out(rgb_info);
        self.transfers.push(rgb_transfer);
        debug!(format!("new transfer out {:#?}", self.transfers));
        self.clone_no_persistence()
    }

    pub fn add_incoming(&mut self, transfer_in: Transfer) -> Self {
        let rgb_transfer = RgbTransfer::unknown(transfer_in);
        self.transfers.push(rgb_transfer);
        debug!(format!("new incoming {:#?}", self.transfers));
        self.clone_no_persistence()
    }

    pub fn update_transfer(&mut self, rgb_info: RgbTransferInfo) -> Self {
        let RgbTransferInfo {
            invoice: rgb_invoice,
            transfer: rgb_transfer,
            contract_amount: new_contract_amount,
            ..
        } = rgb_info.clone();
        let transfer = self
            .clone_no_persistence()
            .transfers
            .into_iter()
            .position(|x| match &x.direction {
                Direction::In(inc) => inc.invoice == rgb_invoice.to_string(),
                Direction::Out(out) => out.transfer_id == rgb_transfer.consignment_id(),
            });

        let rgb_transfer_armor = rgb_transfer.to_ascii_armored_string();
        if let Some(index) = transfer {
            let mut entry = self.transfers.remove(index);

            // Always update the real contract id from the consignment to avoid placeholder leaks
            entry.contract_id = rgb_transfer.contract_id();

            // Only overwrite an existing non-zero amount if the new amount is non-zero
            // This prevents proxy re-scans or zero/placeholder values from wiping a correct amount.
            if let Some(amount) = new_contract_amount {
                use crate::rgb::models::ContractValue;
                match (&entry.contract_amount, &amount) {
                    (Some(ContractValue::Value(prev)), ContractValue::Value(new)) => {
                        let prev_u = prev.parse::<u64>().unwrap_or(0);
                        let new_u = new.parse::<u64>().unwrap_or(0);
                        // Prefer the first non-zero, or upgrade to a non-zero if previous was zero
                        if new_u > 0 || prev_u == 0 {
                            entry.contract_amount = Some(amount);
                        }
                    }
                    // UDA or no previous value: take the new value as-is
                    _ => {
                        entry.contract_amount = Some(amount);
                    }
                }
            }

            match entry.direction.clone() {
                Direction::In(mut inc) => {
                    // Persist the armored consignment
                    inc.transfer = Some(rgb_transfer_armor);
                    entry.direction = Direction::In(inc);

                    // If this is fungible and amount is not yet set, attempt derivation from consignment
                    let is_fungible = self
                        .contract_by(entry.contract_id)
                        .map(|ci| ci.contract_type().is_fungible())
                        .unwrap_or(false);

                    if is_fungible
                        && entry.contract_amount.is_none()
                        && let Some(derived) =
                            derive_fungible_amount_from_consignment(&rgb_transfer)
                    {
                        // Store raw integer value; UI normalization uses precision later
                        entry.contract_amount = Some(ContractValue::value(derived.to_string()));
                    }
                }
                Direction::Out(mut out) => {
                    out.transfer = rgb_transfer_armor;
                    entry.direction = Direction::Out(out);
                }
            }
            debug!(format!(
                "update transfer {} {}",
                rgb_transfer.consignment_id(),
                entry.status.clone()
            ));
            self.transfers.insert(index, entry);
        }
        self.clone_no_persistence()
    }

    pub fn accept_transfer(&mut self, accept_info: AcceptInfo) {
        let rgb_transfer = accept_info.transfer.clone();
        let rgb_transfer_armor = rgb_transfer.to_ascii_armored_string();

        let terminals = rgb_transfer.terminals.clone();
        let mut seals: Vec<SecretSeal> = vec![];
        terminals.into_iter().for_each(|(_, chain_seals)| {
            chain_seals.iter().for_each(|chain_seal| {
                seals.push(*chain_seal);
            });
        });

        let position = self.transfers.iter().position(|x| {
            if !x.is_in() || !x.is_open() {
                return false;
            }
            match &x.direction {
                Direction::In(rgb_in) => {
                    if let Some(transfer) = &rgb_in.transfer {
                        let prev_transfer = Transfer::from_str(transfer).expect("invalid transfer");
                        prev_transfer == accept_info.transfer
                    } else if !rgb_in.invoice.is_empty() {
                        let rgb_invoice =
                            RgbInvoice::from_str(&rgb_in.invoice).expect("invalid invoice");
                        match rgb_invoice.beneficiary.into_inner() {
                            Beneficiary::BlindedSeal(secret_seal) => seals.contains(&secret_seal),
                            _ => false,
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            }
        });

        if let Some(position) = position {
            let mut entry = self.transfers.remove(position);

            // --- COMBINED LOGIC IS HERE ---
            // 1. Update the contract_id (from original accept logic)
            entry.contract_id = accept_info.contract_id;

            // 2. Update the amount if one was provided (from old update_transfer logic)
            if let Some(amount) = accept_info.amount {
                entry.contract_amount = Some(amount);
            }

            // 3. Persist the consignment and mark as finished (from original accept logic)
            entry = entry.complete(rgb_transfer_armor);

            self.transfers.insert(position, entry);
        }
        // No return value, ensuring a true in-place mutation.
    }

    pub fn update_offer(&mut self, offer: RgbOffer) {
        if let Some(position) = self.offers.iter().position(|x| x.id == offer.id) {
            self.offers.remove(position);
            self.offers.insert(position, offer);
        }
    }

    pub fn update_bid(&mut self, bid: RgbBid) {
        if let Some(position) = self.bids.iter().position(|x| x.id == bid.id) {
            self.bids.remove(position);
            self.bids.insert(position, bid);
        }
    }

    pub fn chain_info(&mut self, transfer_id: ConsignmentId, chain_info: ChainInfo) -> Self {
        // Locate *any* transfer (incoming or outgoing, open or finished) that matches the
        // provided transfer id and update its on-chain information. This ensures that the
        // front-end can always display the latest confirmation status once the anchor
        // transaction is detected on-chain.

        // Find the correct position in the original transfers list
        let position = self.transfers.iter().position(|x| {
            // Only consider transfers with transfer_id
            if x.transfer_id().is_none() {
                return false;
            }

            match &x.direction {
                Direction::In(rgb_in) => {
                    if let Some(rgb_transfer) = &rgb_in.transfer {
                        let rgb_transfer =
                            Transfer::from_str(rgb_transfer).expect("invalid transfer");
                        rgb_transfer.consignment_id() == transfer_id
                    } else {
                        false
                    }
                }
                Direction::Out(rgb_out) => {
                    let rgb_transfer =
                        Transfer::from_str(&rgb_out.transfer).expect("invalid transfer");
                    rgb_transfer.consignment_id() == transfer_id
                }
            }
        });

        if let Some(position) = position {
            let mut rgb_transfer = self.transfers.remove(position);
            rgb_transfer.chain_info = Some(chain_info);
            self.transfers.insert(position, rgb_transfer);
        }

        self.clone_no_persistence()
    }

    pub fn add_offer(&mut self, offer: RgbOffer) -> Self {
        self.offers.push(offer);
        self.clone_no_persistence()
    }

    pub fn add_bid(&mut self, bid: RgbBid) -> Self {
        self.bids.push(bid);
        self.clone_no_persistence()
    }
    /// Mark an outgoing transfer as `Finish` once its anchor transaction is mined.
    /// This frees the previously locked UTXO and prevents further
    /// "open_transfer / open_seal" warnings.
    pub fn finalize_outgoing(&mut self, transfer_id: ConsignmentId) -> Self {
        if let Some(pos) = self
            .transfers
            .iter()
            .position(|t| t.is_out() && t.is_open() && (t.transfer_id() == Some(transfer_id)))
        {
            self.transfers[pos].status = Status::Finish;
        }

        self.clone_no_persistence()
    }
}

pub struct FilterTransfer {
    transfer_id: ConsignmentId,
}

impl FilterTransfer {
    pub fn new(transfer_id: ConsignmentId) -> Self {
        Self { transfer_id }
    }
}

pub struct FilterOutpoint {
    outpoint: Outpoint,
    status: Status,
}

impl FilterOutpoint {
    pub fn new(outpoint: Outpoint, status: Status) -> Self {
        Self { outpoint, status }
    }
}

#[derive(Clone, PartialEq)]
pub struct FilterStatus {
    status: Status,
}

impl FilterStatus {
    pub fn new(status: Status) -> Self {
        Self { status }
    }
}

pub trait AccountFilter {
    fn should_include(
        &self,
        outpoint: impl Into<Option<Outpoint>>,
        contract_id: Option<ContractId>,
        transfer_id: Option<ConsignmentId>,
        status: Option<Status>,
    ) -> bool;
}

impl AccountFilter for FilterIncludeAll {
    fn should_include(
        &self,
        _: impl Into<Option<Outpoint>>,
        _: Option<ContractId>,
        _: Option<ConsignmentId>,
        _: Option<Status>,
    ) -> bool {
        true
    }
}
impl AccountFilter for FilterOutpoint {
    fn should_include(
        &self,
        outpoint: impl Into<Option<Outpoint>>,
        _: Option<ContractId>,
        _: Option<ConsignmentId>,
        status: Option<Status>,
    ) -> bool {
        let matches_outpoint = outpoint.into() == Some(self.outpoint);

        let matches_status = status == Some(self.status);

        matches_outpoint && matches_status
    }
}

impl AccountFilter for FilterStatus {
    fn should_include(
        &self,
        _: impl Into<Option<Outpoint>>,
        _: Option<ContractId>,
        _: Option<ConsignmentId>,
        status: Option<Status>,
    ) -> bool {
        status.is_some() && status.unwrap() == self.status
    }
}

impl AccountFilter for FilterTransfer {
    fn should_include(
        &self,
        _: impl Into<Option<Outpoint>>,
        _: Option<ContractId>,
        transfer_id: Option<ConsignmentId>,
        _: Option<Status>,
    ) -> bool {
        transfer_id.is_some() && transfer_id.unwrap() == self.transfer_id
    }
}

impl AccountFilter for Outpoint {
    fn should_include(
        &self,
        outpoint: impl Into<Option<Outpoint>>,
        _: Option<ContractId>,
        _: Option<ConsignmentId>,
        _: Option<Status>,
    ) -> bool {
        let outpoint = outpoint.into();
        outpoint.is_some() && outpoint.unwrap() == *self
    }
}

pub struct OrderBy {
    id: String,
}

impl OrderBy {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

pub trait OfferFilter {
    fn should_include(&self, id: Option<String>, status: Option<Status>) -> bool;
}
impl OfferFilter for FilterIncludeAll {
    fn should_include(&self, _: Option<String>, _: Option<Status>) -> bool {
        true
    }
}

impl OfferFilter for OrderBy {
    fn should_include(&self, id: Option<String>, _: Option<Status>) -> bool {
        id.is_some() && self.id == id.unwrap()
    }
}

pub trait BidFilter {
    fn should_include(
        &self,
        id: Option<String>,
        offer_id: Option<String>,
        status: Option<Status>,
    ) -> bool;
}

impl BidFilter for FilterIncludeAll {
    fn should_include(&self, _: Option<String>, _: Option<String>, _: Option<Status>) -> bool {
        true
    }
}

impl BidFilter for OrderBy {
    fn should_include(&self, id: Option<String>, _: Option<String>, _: Option<Status>) -> bool {
        id.is_some() && self.id == id.unwrap()
    }
}

#[derive(Clone, PartialEq)]
pub struct RgbInvoiceInfo {
    pub seal: String,
    pub precision: u8,
    pub invoice: RgbInvoice,
}

impl RgbInvoiceInfo {
    pub fn new(invoice: RgbInvoice, seal: String, precision: u8) -> Self {
        Self {
            seal,
            precision,
            invoice,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct RgbTransferInfo {
    pub transfer: Transfer,
    pub invoice: RgbInvoice,
    pub psbt: Option<Psbt>,
    pub precision: u8,
    pub contract_amount: Option<ContractValue>,
}

impl RgbTransferInfo {
    pub(crate) fn new(
        rgb_tranfer: Transfer,
        rgb_invoice: RgbInvoice,
        precision: u8,
        psbt: Option<Psbt>,
    ) -> Self {
        Self {
            transfer: rgb_tranfer,
            invoice: rgb_invoice,
            psbt,
            precision,
            contract_amount: None,
        }
    }

    pub(crate) fn with(rgb_tranfer: Transfer, rgb_invoice: RgbInvoice) -> Self {
        Self {
            transfer: rgb_tranfer,
            invoice: rgb_invoice,
            psbt: none!(),
            precision: 0,
            contract_amount: None,
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct ChainInfo {
    pub txid: Txid,
    pub status: TxStatus,
}

impl ChainInfo {
    pub fn with(txid: Txid, status: TxStatus) -> Self {
        Self { txid, status }
    }

    pub fn mempool(txid: Txid) -> Self {
        Self {
            txid,
            status: TxStatus::Mempool,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RgbTransfer {
    pub contract_id: ContractId,
    pub status: Status,
    pub direction: Direction,
    pub chain_info: Option<ChainInfo>,
    pub contract_amount: Option<ContractValue>,
}

impl RgbTransfer {
    fn new_in(invoice_info: RgbInvoiceInfo) -> Self {
        let RgbInvoiceInfo {
            invoice: rgb_invoice,
            ..
        } = invoice_info.clone();

        let contract_amount = match rgb_invoice.assignment_state {
            // Persist raw base units (u64) to avoid non-numeric strings like "aa"
            Some(InvoiceState::Amount(amount)) => ContractValue::value(amount.value().to_string()),
            Some(InvoiceState::Data(NonFungible::FractionedToken(allocation))) => {
                ContractValue::uda(allocation.into())
            }
            _ => panic!("wrong invoice state"),
        };

        Self {
            contract_id: rgb_invoice.contract.unwrap_or(ContractId::strict_dumb()),
            direction: Direction::In(RgbIn::new(invoice_info)),
            status: Status::Open,
            contract_amount: Some(contract_amount),
            chain_info: None,
        }
    }

    fn new_out(rgb_info: RgbTransferInfo) -> Self {
        let RgbTransferInfo {
            invoice: rgb_invoice,
            transfer: rgb_transfer,
            ..
        } = rgb_info.clone();
        let contract_amount = match rgb_invoice.assignment_state {
            // Persist raw base units (u64) to avoid non-numeric strings like "aa"
            Some(InvoiceState::Amount(amount)) => ContractValue::value(amount.value().to_string()),
            Some(InvoiceState::Data(NonFungible::FractionedToken(allocation))) => {
                ContractValue::uda(allocation.into())
            }
            _ => panic!("wrong invoice state"),
        };

        let (txid, _) = extract_transfer(&rgb_transfer).expect("invalid transfer");

        Self {
            status: Status::Open,
            contract_amount: Some(contract_amount),
            contract_id: rgb_transfer.contract_id(),
            direction: Direction::Out(RgbOut::with(rgb_transfer, rgb_invoice)),
            chain_info: Some(ChainInfo::mempool(txid)),
        }
    }

    fn unknown(transfer_info: Transfer) -> Self {
        Self {
            contract_id: transfer_info.contract_id(),
            direction: Direction::In(RgbIn::with(transfer_info)),
            status: Status::Open,
            contract_amount: None,
            chain_info: None,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.status == Status::Finish
    }

    fn complete(mut self, cp: String) -> Self {
        match self.direction {
            Direction::In(mut inc) => {
                inc.transfer = Some(cp);
                self.direction = Direction::In(inc);
                self.status = Status::Finish;

                self
            }
            Direction::Out(out) => {
                // Do NOT overwrite the original invoice with the consignment.
                // Keeping the invoice string intact allows proxy lookups for sender amounts.
                // If needed, the consignment armor is already stored in `out.transfer`.
                self.direction = Direction::Out(out);
                self.status = Status::Finish;

                self
            }
        }
    }
}

impl RgbTransfer {
    pub fn transfer_id(&self) -> Option<ConsignmentId> {
        match self.clone().direction {
            Direction::In(inc) => inc
                .transfer
                .map(|transfer| Transfer::from_str(&transfer).unwrap().consignment_id()),
            Direction::Out(out) => ConsignmentId::from_str(&out.transfer_id.to_string()).ok(),
        }
    }

    pub fn has_invoice(&self, rgb_invoice: RgbInvoice) -> bool {
        if let Direction::In(inc) = self.direction.clone() {
            inc.invoice.eq(&rgb_invoice.to_string())
        } else {
            false
        }
    }

    pub fn is_in(&self) -> bool {
        matches!(self.direction, Direction::In(_))
    }

    pub fn is_out(&self) -> bool {
        matches!(self.direction, Direction::Out(_))
    }

    pub fn is_open(&self) -> bool {
        matches!(self.status, Status::Open)
    }

    pub fn is_payed(&self) -> bool {
        matches!(self.status, Status::Finish)
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct RgbIn {
    pub utxo: String,
    pub invoice: String,
    pub transfer: Option<String>,
}

impl RgbIn {
    fn new(rgb_invoice: RgbInvoiceInfo) -> RgbIn {
        Self {
            utxo: rgb_invoice.seal,
            invoice: rgb_invoice.invoice.to_string(),
            transfer: None,
        }
    }

    fn with(rgb_transfer: Transfer) -> RgbIn {
        Self {
            utxo: String::new(),
            invoice: String::new(),
            transfer: Some(rgb_transfer.to_ascii_armored_string()),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct RgbOut {
    pub transfer_id: ConsignmentId,
    pub transfer: String,
    pub invoice: String,
}

impl RgbOut {
    fn with(rgb_transfer: Transfer, rgb_invoice: RgbInvoice) -> RgbOut {
        let rgb_transfer_armor = rgb_transfer.to_ascii_armored_string();
        Self {
            transfer_id: rgb_transfer.consignment_id(),
            transfer: rgb_transfer_armor,
            invoice: rgb_invoice.to_string(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize, Display)]
#[display(doc_comments)]
pub enum Status {
    #[display("open")]
    Open,
    #[display("pending")]
    Pending,
    #[display("finish")]
    Finish,
    #[display("cancel")]
    Cancel,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum Direction {
    In(RgbIn),
    Out(RgbOut),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AcceptInfo {
    pub contract_id: ContractId,
    #[serde(with = "crate::rgb::compat::transfer_serde")]
    pub transfer: Transfer,
    // ADD THIS NEW FIELD:
    pub amount: Option<ContractValue>,
}

impl AcceptInfo {
    // Modify the `new` function to accept the optional amount.
    pub fn new(contract_id: ContractId, transfer: Transfer, amount: Option<ContractValue>) -> Self {
        Self {
            contract_id,
            transfer,
            amount,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub struct RgbOrderbooks {
    pub offers: BTreeMap<ContractId, Vec<PublicRgbOffer>>,
    pub bids: BTreeMap<OrderId, BTreeMap<OrderId, PublicRgbBid>>,
}

impl RgbOrderbooks {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Getters, Serialize, Deserialize, Debug)]
pub struct RgbOffer {
    id: OrderId,
    status: Status,
    asset: ContractMeta,
    amount: ContractAmount,
    counter_party: CounterParty,
    suppl: Option<OrderSuppl>,
    swap: Option<SwapSupplement>,
}

impl RgbOffer {
    pub fn is_open(&self) -> bool {
        matches!(self.status, Status::Open)
    }

    pub fn contract_id(&self) -> ContractId {
        self.asset.info.contract_id
    }

    pub fn ty(&self) -> AssetType {
        self.asset.info.contract_type.clone()
    }

    pub fn is_contract(&self) -> bool {
        matches!(self.counter_party, CounterParty::Contract(_))
    }

    pub fn is_sats(&self) -> bool {
        matches!(self.counter_party, CounterParty::Sats(_))
    }

    pub fn psbt_partial(&mut self) -> Option<Psbt> {
        match self.suppl.clone() {
            Some(order) => Psbt::from_str(&order.psbt).ok(),
            _ => None,
        }
    }

    pub fn complete(&mut self) {
        self.status = Status::Finish;
    }

    pub fn cancel(&mut self) {
        self.status = Status::Cancel;
    }

    pub fn add_suppl(&mut self, suppl: OrderSuppl) {
        self.suppl = Some(suppl);
    }

    pub fn add_swap_suppl(&mut self, swap_suppl: SwapSupplement) {
        self.swap = Some(swap_suppl);
    }

    pub(crate) fn to_public(&self, armored: String) -> PublicRgbOffer {
        let RgbOffer {
            id,
            status,
            counter_party,
            asset: contract,
            amount: contract_amount,
            suppl,
            swap,
            ..
        } = self.clone();

        let contract = ContractKit::new(contract.info, armored);
        PublicRgbOffer {
            id,
            status,
            counter_party,
            asset: contract,
            amount: contract_amount,
            order_suppl: suppl,
            swap_suppl: swap,
        }
    }

    pub fn new(
        contract_amount: ContractAmount,
        contract_meta: ContractMeta,
        counter_party: CounterParty,
    ) -> RgbOffer {
        let mut rng = StdRng::from_entropy();
        let mut data = [0u8; 8];
        rng.fill_bytes(&mut data);

        let mut hasher = blake3::Hasher::new();
        hasher.update(data.as_slice());

        let id = Array::from_array(hasher.finalize().into());
        let order_id = OrderId(id);

        Self {
            id: order_id,
            status: Status::Open,
            asset: contract_meta,
            amount: contract_amount,
            counter_party,
            swap: None,
            suppl: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PublicRgbOffer {
    pub id: OrderId,
    pub status: Status,
    pub amount: ContractAmount,
    pub asset: ContractKit,
    pub counter_party: CounterParty,
    pub order_suppl: Option<OrderSuppl>,
    pub swap_suppl: Option<SwapSupplement>,
}

impl PublicRgbOffer {
    pub fn contract_id(&self) -> ContractId {
        self.asset.info.contract_id
    }
    pub fn is_open(&self) -> bool {
        matches!(self.status, Status::Open)
    }

    pub fn is_fill(&self) -> bool {
        matches!(self.status, Status::Finish)
    }

    pub fn complete(&mut self) {
        self.status = Status::Finish;
    }

    pub fn cancel(&mut self) {
        self.status = Status::Cancel;
    }
}

#[derive(Clone, Getters, Serialize, Deserialize, Debug)]
pub struct RgbBid {
    id: OrderId,
    offer_id: OrderId,
    status: Status,
    amount: ContractAmount,
    asset: BidAsset,
    counter_party: CounterParty,
    suppl: Option<OrderSuppl>,
    swap: Option<SwapSupplement>,
}

impl RgbBid {
    pub fn is_open(&self) -> bool {
        matches!(self.status, Status::Open)
    }

    pub fn is_contract(&self) -> bool {
        matches!(self.asset, BidAsset::Contract(_))
    }

    pub fn is_sats(&self) -> bool {
        matches!(self.asset, BidAsset::L1(_))
    }

    pub fn contract_id(&self) -> Option<ContractId> {
        match &self.asset {
            BidAsset::Contract(contract) => Some(contract.info.contract_id),
            _ => None,
        }
    }

    pub fn l1_amount(&self) -> Sats {
        match &self.asset {
            BidAsset::L1(l1) => l1.sats,
            _ => Sats::default(),
        }
    }

    pub fn contract_type(&self) -> Option<AssetType> {
        match &self.asset {
            BidAsset::Contract(contract) => Some(contract.info.contract_type.clone()),
            _ => None,
        }
    }

    pub fn inputs(&self) -> Vec<UtxoKit> {
        match &self.asset {
            BidAsset::Contract(contract) => contract.inputs.clone(),
            BidAsset::L1(layer1) => layer1.inputs.clone(),
        }
    }

    pub fn psbt_partial(&mut self) -> Option<Psbt> {
        match self.suppl.clone() {
            Some(order) => Psbt::from_str(&order.psbt).ok(),
            _ => None,
        }
    }

    pub fn complete(&mut self) {
        self.status = Status::Finish;
    }

    pub fn cancel(&mut self) {
        self.status = Status::Cancel;
    }

    pub fn add_suppl(&mut self, suppl: OrderSuppl) {
        self.suppl = Some(suppl);
    }

    pub fn add_swap(&mut self, suppl: SwapSupplement) {
        self.swap = Some(suppl);
    }

    pub(crate) fn to_public(&self, _: String) -> PublicRgbBid {
        let RgbBid {
            id,
            offer_id,
            amount: contract_amount,
            status,
            asset,
            swap,
            counter_party,
            suppl,
            ..
        } = self.clone();

        PublicRgbBid {
            id,
            offer_id,
            status,
            asset: asset.into(),
            amount: contract_amount,
            counter_party,
            swap,
            suppl,
        }
    }

    pub fn new(offer: PublicRgbOffer, _bid_value: OrderValue, asset: BidAsset) -> RgbBid {
        let mut rng = StdRng::from_entropy();
        let mut data = [0u8; 8];
        rng.fill_bytes(&mut data);

        let mut hasher = blake3::Hasher::new();
        hasher.update(data.as_slice());

        let id = Array::from_array(hasher.finalize().into());
        let order_id = OrderId(id);

        let counter_party = CounterParty::Contract(ContractSwap::with(offer.asset, offer.amount));

        let amount = match offer.counter_party {
            // OrderValue::Bitcoin(_) => ContractAmount::new(0, 0, 0),
            // OrderValue::Contract(contract_swap) => contract_swap.amount,
            CounterParty::Sats(_) => ContractAmount::new(0, 0, 0),
            CounterParty::Contract(contract_swap) => contract_swap.amount,
        };

        Self {
            id: order_id,
            offer_id: offer.id,
            status: Status::Open,
            asset,
            amount,
            counter_party,
            swap: None,
            suppl: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum AssetKit {
    Bitcoin,
    Contract(ContractKit),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PublicRgbBid {
    pub id: OrderId,
    pub offer_id: OrderId,
    pub status: Status,
    pub asset: AssetKit,
    pub amount: ContractAmount,
    pub counter_party: CounterParty,
    pub suppl: Option<OrderSuppl>,
    pub swap: Option<SwapSupplement>,
}

impl PublicRgbBid {
    pub fn is_open(&self) -> bool {
        matches!(self.status, Status::Open)
    }

    pub fn counter_party(&self) -> Option<ContractId> {
        match &self.counter_party {
            CounterParty::Contract(contract) => Some(contract.contract_id()),
            _ => None,
        }
    }

    pub fn complete(&mut self) {
        self.status = Status::Finish;
    }
}

#[derive(
    Wrapper,
    Copy,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    Debug,
    Serialize,
    Deserialize,
    From,
    Default,
)]
#[wrapper(Deref, BorrowSlice, Hex, Index, RangeOps)]
pub struct OrderId(
    #[from]
    #[from([u8; 32])]
    Bytes32,
);

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum CounterParty {
    Sats(u64),
    Contract(ContractSwap),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ContractSwap {
    pub kit: ContractKit,
    pub amount: ContractAmount,
}

impl ContractSwap {
    pub fn contract_id(&self) -> ContractId {
        self.kit.info.contract_id
    }
    pub fn with(kit: ContractKit, amount: ContractAmount) -> Self {
        Self { kit, amount }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum BidAsset {
    L1(Layer1Meta),
    Contract(ContractMeta),
}

#[derive(Clone, Getters, Serialize, Deserialize, Debug)]
pub struct Layer1Meta {
    inputs: Vec<UtxoKit>,
    sats: Sats,
}

impl Layer1Meta {
    pub fn with(sats: Sats, inputs: Vec<UtxoKit>) -> Self {
        Self { sats, inputs }
    }
}

#[derive(Clone, Getters, Serialize, Deserialize, Debug)]
pub struct ContractMeta {
    info: ContractInfo,
    inputs: Vec<UtxoKit>,
}

impl ContractMeta {
    pub fn contract_id(&self) -> ContractId {
        self.info.contract_id
    }

    pub fn with(info: ContractInfo, inputs: Vec<UtxoKit>) -> Self {
        Self { info, inputs }
    }
}

#[derive(Hash, Ord, Eq, PartialOrd, PartialEq, Clone, Serialize, Deserialize, Debug)]
pub struct UtxoKit {
    pub outpoint: Outpoint,
    pub value: Sats,
    pub terminal: Terminal,
    pub status: TxStatus,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OutpointKit {
    pub outpoint: Outpoint,
    pub value: Sats,
}

#[derive(
    Clone, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Display, Default,
)]
#[display("{info:?}")]
pub struct ContractKit {
    pub info: ContractInfo,
    pub armored: String,
}

impl ContractKit {
    pub fn new(info: ContractInfo, armored: String) -> Self {
        Self { info, armored }
    }
}

// Pending-host helpers: allow exporters and runtime to mark ownership by derivation+tapret
impl RgbAccount {
    pub fn add_pending_host(&mut self, terminal: Terminal, outpoint: impl ToString) -> Self {
        let key = outpoint.to_string();

        // Only insert if NOT already present - first-write-wins to prevent terminal conflicts
        if let Some(existing) = self.pending_hosts.get(&key) {
            if existing != &terminal {
                crate::debug!(format!(
                    "pending_host.conflict outpoint={} existing_terminal={} new_terminal={} action=keep_existing",
                    key, existing, terminal
                ));
            }
            // Silently keep existing - no error, no overwrite
        } else {
            self.pending_hosts.insert(key.clone(), terminal);
            crate::debug!(format!(
                "pending_host.added outpoint={} terminal={}",
                key, terminal
            ));
        }

        self.clone_no_persistence()
    }

    pub fn is_pending_host_outpoint(&self, outpoint: &str) -> Option<Terminal> {
        self.pending_hosts.get(outpoint).cloned()
    }

    // Expose read-only view of pending hosts for exporters
    pub fn pending_hosts(&self) -> &BTreeMap<String, Terminal> {
        &self.pending_hosts
    }

    // Expose read-only view of tapret commitments per terminal
    pub fn tapret_commits(&self) -> &BTreeMap<Terminal, TapretCommitment> {
        &self.commits
    }

    // Query if we have a tapret commit stored for a given terminal
    pub fn has_tapret_for_terminal(&self, terminal: &Terminal) -> bool {
        self.commits.contains_key(terminal)
    }

    // Note: safe no-op for now; wallet indexing will naturally converge and callers may choose to clear.
    // We avoid parsing Outpoint here to keep types consistent with multiple Outpoint definitions across crates.
    pub fn clear_pending_host_entry(&mut self, outpoint: &str) -> bool {
        self.pending_hosts.remove(outpoint).is_some()
    }

    // Clean up stale pending_hosts: remove entries for UTXOs that are now properly indexed
    // This prevents cache inconsistency errors when bp-wallet tries to reconcile stale metadata
    // Should be called after wallet load to ensure clean state before any balance() calls
    pub fn cleanup_stale_pending_hosts(&mut self) {
        use std::str::FromStr;

        let pending_hosts_before = self.pending_hosts.clone();
        let mut cleaned_count = 0;
        for outpoint_str in pending_hosts_before.keys() {
            if let Ok(outpoint) = bpwallet::Outpoint::from_str(outpoint_str.as_str()) {
                // If wallet can now resolve this outpoint, it's been indexed - remove from pending_hosts
                if self.wallet.outpoint_by(outpoint).is_ok() {
                    self.clear_pending_host_entry(outpoint_str);
                    cleaned_count += 1;
                    crate::debug!(format!(
                        "pending_hosts.cleanup.removed outpoint={} reason=now_indexed",
                        outpoint_str
                    ));
                }
            }
        }
        if cleaned_count > 0 {
            crate::debug!(format!(
                "pending_hosts.cleanup.summary removed={} remaining={}",
                cleaned_count,
                self.pending_hosts.len()
            ));
        }
    }

    // Durable lookahead hinting to avoid gap-limit blindness after restart
    pub fn add_lookahead_hint(&mut self, keychain: Keychain, index: NormalIndex) -> Self {
        let entry = self.lookahead_hints.entry(keychain).or_insert(index);
        // keep the max index so we don't regress
        if index > *entry {
            *entry = index;
        }
        self.clone_no_persistence()
    }

    pub fn lookahead_hints(&self) -> &BTreeMap<Keychain, NormalIndex> {
        &self.lookahead_hints
    }
}
