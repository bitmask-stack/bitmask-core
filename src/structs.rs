use garde::Validate;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::rgb::models::{AssetKit, ContractKit, UtxoKit};
use crate::{
    rgb::models::ContractAmount,
    rgb::swap::RgbSwap,
    validators::{RGBValidationOptions, decimal_repr, rgb_select_option},
};
use bitcoin::{Address, Transaction, Txid, address::NetworkUnchecked};

/// Wallet balances in satoshis. Same camelCase JSON as BDK 0.28 `Balance`
/// (u64 fields). Do not serialize `bdk_wallet::Balance` (`Amount`) here.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub immature: u64,
    pub trusted_pending: u64,
    pub untrusted_pending: u64,
    pub confirmed: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockTime {
    pub height: u32,
    pub timestamp: u64,
}

/// Product transaction details. Same camelCase JSON as BDK 0.28
/// `TransactionDetails` (u64 sent/received/fee).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDetails {
    pub transaction: Option<Transaction>,
    pub txid: Txid,
    pub received: u64,
    pub sent: u64,
    pub fee: Option<u64>,
    pub confirmation_time: Option<BlockTime>,
}
use rgb::invoice::{InvoiceState, NonFungible, RgbInvoice};
use rgbstd::containers::Transfer;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletData {
    pub address: String,
    pub balance: Balance,
    pub transactions: Vec<WalletTransaction>,
    pub utxos: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WalletTransaction {
    pub txid: Txid,
    pub received: u64,
    pub sent: u64,
    pub fee: Option<u64>,
    pub confirmed: bool,
    pub confirmation_time: Option<BlockTime>,
    pub vsize: usize,
    pub fee_rate: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionData {
    pub details: TransactionDetails,
    pub vsize: usize,
    pub fee_rate: f32,
}

#[derive(
    Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop, Display, Eq, PartialEq, Default,
)]
#[display(inner)]
pub struct SecretString(pub String);

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct PrivateWalletData {
    pub xprvkh: String,
    pub btc_descriptor_xprv: String,
    pub btc_change_descriptor_xprv: String,
    pub rgb_descriptor_xprv: String,
    pub nostr_prv: String,
    pub nostr_nsec: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct PublicWalletData {
    pub xpubkh: String,
    pub btc_descriptor_xpub: String,
    pub btc_change_descriptor_xpub: String,
    pub rgb_descriptor_xpub: String,
    pub nostr_pub: String,
    pub nostr_npub: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
pub struct DecryptedWalletData {
    pub mnemonic: String,
    pub private: PrivateWalletData,
    pub public: PublicWalletData,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedWalletDataV04 {
    pub btc_descriptor_xprv: String,
    pub btc_descriptor_xpub: String,
    pub btc_change_descriptor_xprv: String,
    pub btc_change_descriptor_xpub: String,
    pub rgb_assets_descriptor_xprv: String,
    pub rgb_assets_descriptor_xpub: String,
    pub rgb_udas_descriptor_xprv: String,
    pub rgb_udas_descriptor_xpub: String,
    pub xprvkh: String,
    pub xpubkh: String,
    pub mnemonic: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FundVaultDetails {
    pub rgb_output: Option<String>,
    pub is_funded: bool,
    pub fund_txid: Option<String>,
    pub fund_fee: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecoverVaultDetails {
    pub txid: Option<String>,
    pub fee: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SatsInvoice {
    pub amount: u64,
    /// Unchecked so JSON round-trips (`Address<NetworkChecked>` has no Deserialize).
    pub address: Address<NetworkUnchecked>,
}

pub struct ContainerResponse<T> {
    pub data: T,
    pub container: Option<RgbContainer>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RgbContainer {
    pub rgb_stock: Option<String>,
    pub rgb_account: Option<String>,
    pub rgb_transfers: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub struct IssueRequest {
    /// The ticker of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 8))]
    pub ticker: String,
    /// Name of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 40))]
    pub name: String,
    /// Iface (ex: RGB20)
    #[garde(alphanumeric)]
    pub iface: String,
    /// Description of the asset
    #[garde(ascii)]
    #[garde(length(min = u8::MIN.into(), max = u8::MAX.into()))]
    pub description: String,
    /// Amount of the asset
    #[garde(ascii)]
    pub supply: String,
    /// Precision of the asset
    #[garde(range(min = u8::MIN, max = u8::MAX))]
    pub precision: u8,
    /// Seal of the initial owner
    #[garde(ascii)]
    pub seal: String,
    /// Chain Target
    #[garde(skip)]
    pub chain: ChainTarget,
    /// contract metadata (only RGB21/UDA)
    #[garde(dive)]
    pub meta: Option<IssueMedia>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub struct AtomicMintRequest {
    /// The ticker of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 8))]
    pub ticker: String,
    /// Name of the asset
    #[garde(ascii)]
    #[garde(length(min = 1, max = 40))]
    pub name: String,
    /// Iface (ex: RGB20)
    #[garde(alphanumeric)]
    pub iface: String,
    /// Description of the asset
    #[garde(ascii)]
    #[garde(length(min = u8::MIN.into(), max = u8::MAX.into()))]
    pub description: String,
    /// Chain Target
    #[garde(skip)]
    pub chain: ChainTarget,
    /// contract metadata (only RGB21/UDA)
    #[garde(dive)]
    pub meta: Option<IssueMedia>,
    /// Bitcoin Price
    #[garde(range(min = 0, max = u64::MAX))]
    pub bitcoin_price: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub struct LoadWalletsRequest {
    /// Number of wallets to request
    #[garde(range(min = u16::MIN, max = u16::MAX))]
    pub number_of_wallets: u16,
    /// Chain Target
    #[garde(skip)]
    pub chain: ChainTarget,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct IssueResponse {
    /// The contract id
    pub contract_id: String,
    /// The contract interface
    pub iface: String,
    /// The Issue Utxo
    pub issue_utxo: String,
    /// The ticker of the asset
    pub ticker: String,
    /// Name of the asset
    pub name: String,
    /// creation date (timestamp)
    pub created: i64,
    /// Description of the asset
    pub description: String,
    /// Amount of the asset
    pub supply: u64,
    /// Precision of the asset
    pub precision: u8,
    /// Amount of the asset
    pub balance: ContractValue,
    /// The contract state (multiple formats)
    pub contract: String,
    /// The contract allocations
    pub allocations: Vec<ContractAllocation>,
    /// contract metadata (only RGB21/UDA)
    pub meta: Option<UDAItem>,
}

impl IssueResponse {
    pub fn to_normalize_balance(&self) -> f64 {
        let resp = match &self.balance {
            ContractValue::Value(amount) => ContractAmount::with(*amount, self.precision)
                .to_string()
                .parse(),
            ContractValue::UDA(allocation) => {
                ContractAmount::with(allocation.fraction, self.precision)
                    .to_string()
                    .parse()
            }
        };

        resp.unwrap_or(0.0)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AtomicMintResponse {
    /// IssueResponse
    pub issue_response: IssueResponse,
    /// SwapOfferResponse
    pub rgb_offer_response: Vec<RgbOfferResponse>,
}

pub type ContractResponse = IssueResponse;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UDAItem {
    // Token Index
    pub token_index: u32,
    /// media of the uda
    pub media: IssueMedia,
}

impl UDAItem {
    pub fn with(token_index: u32, media: IssueMedia) -> Self {
        Self { token_index, media }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContractFormats {
    /// The contract state (encoded in hexadecimal)
    pub strict: String,
    /// The contract state (compiled in armored mode)
    pub armored: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenesisFormats {
    /// The genesis state (encoded in hexadecimal)
    pub strict: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub struct IssueMedia {
    #[garde(dive)]
    pub preview: Option<MediaData>,
    #[garde(dive)]
    pub media: Option<MediaData>,
    #[garde(dive)]
    pub attachments: Vec<MediaData>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub struct MediaData {
    /// Media Ty
    #[garde(ascii)]
    #[garde(length(min = 1, max = 64))]
    #[serde(rename = "type")]
    pub ty: String,
    /// Media Uri/Source
    #[garde(ascii)]
    #[garde(length(min = u16::MIN.into(), max = u16::MAX.into()))]
    pub uri: String,
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContractAllocation {
    /// Anchored UTXO
    pub utxo: String,
    /// Asset Value
    pub value: ContractValue,
    /// Derivation Path
    pub derivation: String,
    /// My Allocation?
    pub is_mine: bool,
    /// Allocation spent?
    pub is_spent: bool,
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone, Display)]
#[serde(rename_all = "camelCase")]
pub enum ContractValue {
    #[display(inner)]
    #[serde(rename = "value")]
    Value(u64),
    #[display(inner)]
    #[serde(rename = "uda")]
    UDA(UDAValue),
}

impl Default for ContractValue {
    fn default() -> Self {
        ContractValue::Value(0)
    }
}

#[derive(
    Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone, Display, Validate,
)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
#[display("{token_index}@{fraction}")]
pub struct UDAValue {
    #[garde(range(min = 1, max = u32::MAX))]
    pub token_index: u32,
    #[garde(range(min = 1, max = u64::MAX))]
    pub fraction: u64,
}

impl Default for UDAValue {
    fn default() -> Self {
        Self {
            token_index: 1,
            fraction: 1,
        }
    }
}

impl UDAValue {
    pub fn with(token_index: u32, fraction: u64) -> Self {
        Self {
            token_index,
            fraction,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub struct RgbInvoiceRequest {
    /// The contract id
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: Option<String>,
    /// Amount of the asset
    #[garde(dive)]
    pub amount: RgbInvoiceValue,
    /// Blinded UTXO
    #[garde(custom(rgb_select_option))]
    pub seal: RgbSelection,
    /// Expiration
    #[garde(range(min = i64::MIN, max = i64::MAX))]
    pub expire_at: Option<i64>,
}

#[derive(
    Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone, Display, Validate,
)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
#[display("{0:?}")]
pub enum RgbSelection {
    /// Chain/Next UTXO
    #[serde(rename = "next_utxo")]
    NextUTXO,
    #[serde(rename = "next_address")]
    NextAddress,
    #[serde(rename = "utxo")]
    UTXO(#[garde(skip)] String),
    /// Chain/Next Address
    #[serde(rename = "address")]
    Address(#[garde(skip)] String),
}

impl RgbSelection {
    pub fn is_address(&self) -> bool {
        matches!(self, Self::Address(_)) || matches!(self, Self::NextAddress)
    }

    pub fn is_utxo(&self) -> bool {
        matches!(self, Self::UTXO(_)) || matches!(self, Self::NextUTXO)
    }
}

#[derive(
    Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone, Display, Validate,
)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub enum RgbInvoiceValue {
    #[display(inner)]
    #[serde(rename = "value")]
    Value(#[garde(custom(decimal_repr))] String),
    #[serde(rename = "uda")]
    UDA(#[garde(dive)] UDAValue),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RgbInvoiceResponse {
    /// Invoice encoded in Baid58
    pub invoice: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RgbInvoiceDecoded {
    /// ContractID
    pub contract_id: String,
    /// Invoice Amount
    pub amount: RgbInvoiceValue,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RgbInvoiceStatusResponse {
    /// Asset Name
    pub asset_name: String,
    /// Asset Name
    pub asset_ticker: String,
    /// ContractID
    pub contract_id: String,
    /// Invoice Amount
    pub amount: RgbInvoiceValue,
    /// Invoice Status
    pub status: RgbInvoiceStatus,
    /// Invoice Encoded
    pub invoice: String,
}

impl RgbInvoiceStatusResponse {
    pub fn new(
        invoice: &RgbInvoice,
        status: &bool,
        asset_name: &str,
        asset_ticker: &str,
        asset_precision: &u8,
    ) -> Self {
        let RgbInvoice {
            contract,
            assignment_state,
            ..
        } = invoice.clone();

        let contract_id = contract.map_or(String::new(), |x| x.to_string());
        let status = if *status {
            RgbInvoiceStatus::Payed
        } else {
            RgbInvoiceStatus::Opened
        };

        let amount = match assignment_state {
            Some(InvoiceState::Amount(amount)) => RgbInvoiceValue::Value(
                ContractAmount::with(amount.value(), *asset_precision).to_string(),
            ),
            Some(InvoiceState::Data(NonFungible::FractionedToken(allocation))) => {
                RgbInvoiceValue::UDA(UDAValue::with(
                    allocation.token_index().into(),
                    allocation.fraction().into(),
                ))
            }
            _ => RgbInvoiceValue::Value("0".to_string()),
        };

        Self {
            contract_id,
            amount,
            status,
            invoice: invoice.to_string(),
            asset_ticker: asset_ticker.to_string(),
            asset_name: asset_name.to_string(),
        }
    }

    pub fn unknown(invoice: &RgbInvoice, status: &bool) -> Self {
        RgbInvoiceStatusResponse::new(invoice, status, default!(), default!(), &u8::MIN)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Display)]
#[serde(rename_all = "camelCase")]
pub enum RgbInvoiceStatus {
    #[default]
    #[display("opened")]
    Opened,
    #[display("payed")]
    Payed,
    #[display("canceled")]
    Canceled,
}

#[derive(Serialize, Deserialize, Debug, Clone, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub struct RgbTransferRequest {
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: String,
    /// The contract interface
    #[garde(ascii)]
    #[garde(length(min = 0, max = 32))]
    pub iface: String,
    /// RGB Invoice
    #[garde(ascii)]
    #[garde(length(min = 0, max = 512))]
    pub invoice: String,
    /// Optional amount provided by the sender for blinded invoices (ignored if invoice carries amount/allocation)
    #[garde(dive)]
    pub sender_amount: Option<RgbInvoiceValue>,
    /// Chain Target
    #[garde(skip)]
    pub chain: ChainTarget,
    /// Chain Fee
    #[garde(dive)]
    pub chain_fee: ChainFee,
}

#[derive(Serialize, Deserialize, Debug, Clone, Validate, Display)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub enum ChainFee {
    #[display("value")]
    Value(#[garde(range(min = 0, max = u64::MAX))] u64),
    #[display("feeRate")]
    FeeRate(#[garde(skip)] f32),
    #[display("none")]
    None,
}

impl Default for ChainFee {
    fn default() -> Self {
        Self::Value(0)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RgbTransferResponse {
    /// Consignment ID (in base58)
    pub consig_id: String,
    /// Consignment (in armored)
    pub consig: String,
    /// Transfer Bitcoin L1 transaction id
    pub txid: String,
    /// PSBT File (in base64)
    pub psbt: String,
    /// Tapret Commitment
    pub commit: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default, Display)]
#[serde(rename_all = "camelCase")]
pub enum ChainTarget {
    #[default]
    #[display("bitcoin")]
    Bitcoin,
}

#[derive(Serialize, Deserialize, Debug, Clone, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(context(RGBValidationOptions))]
pub struct AcceptRequest {
    /// Consignment encoded in hexadecimal
    #[garde(ascii)]
    #[garde(length(min = 0, max = usize::MAX))]
    pub consignment: String,
    /// Force Consignment accept
    #[garde(skip)]
    pub force: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AcceptResponse {
    /// Transfer ID (Consig ID)
    pub transfer_id: String,
    /// Contract ID
    pub contract_id: String,
    /// Anchor Bitcoin transaction id
    pub txid: String,
    /// Transfer accept status
    pub valid: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(context(RGBValidationOptions))]
pub struct SaveTransferRequest {
    /// Consignment encoded in hexadecimal
    #[garde(ascii)]
    #[garde(length(min = 0, max = usize::MAX))]
    pub consignment: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RgbTransferItem {
    pub contract_id: String,
    pub iface: String,
    pub direction: Direction,
}

impl RgbTransferItem {
    pub fn incoming(contract_id: &str, iface: &str, transfer: TransferIn) -> Self {
        Self {
            contract_id: contract_id.to_string(),
            iface: iface.to_string(),
            direction: Direction::In(transfer),
        }
    }

    pub fn outcoming(contract_id: &str, iface: &str, transfer: TransferOut) -> Self {
        Self {
            contract_id: contract_id.to_string(),
            iface: iface.to_string(),
            direction: Direction::Out(transfer),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    #[serde(rename = "in")]
    In(TransferIn),
    #[serde(rename = "out")]
    Out(TransferOut),
}

impl Default for Direction {
    fn default() -> Self {
        Self::In(TransferIn::default())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TransferIn {
    pub invoice: String,
    pub tx_id: String,
    pub tx_status: TxStatus,
    pub transfer: Option<TransferItem>,
    pub amount: Option<ContractAmount>,
    // RGB21 (UDA) display value (token_index/fraction) — optional alongside fungible amount
    pub uda: Option<UDAValue>,
}

impl TransferIn {
    pub fn with(
        invoice: String,
        tx_id: String,
        tx_status: TxStatus,
        transfer: Option<TransferItem>,
        amount: Option<ContractAmount>,
    ) -> Self {
        Self {
            invoice,
            tx_id,
            tx_status,
            transfer,
            amount,
            uda: None,
        }
    }

    pub fn unknown(invoice: String, tx_id: String, transfer: Option<TransferItem>) -> Self {
        Self {
            invoice,
            tx_id,
            transfer,
            tx_status: TxStatus::NotFound,
            amount: None,
            uda: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransferOut {
    pub invoice: String,
    pub tx_id: String,
    pub tx_status: TxStatus,
    pub transfer: TransferItem,
    pub amount: Option<ContractAmount>,
    // RGB21 (UDA) display value (token_index/fraction) — optional alongside fungible amount
    pub uda: Option<UDAValue>,
}

impl TransferOut {
    pub fn with(
        invoice: String,
        transfer: TransferItem,
        tx_id: String,
        tx_status: TxStatus,
        amount: Option<ContractAmount>,
    ) -> Self {
        Self {
            tx_id,
            tx_status,
            invoice,
            transfer,
            amount,
            uda: None,
        }
    }

    pub fn unknown(invoice: String, transfer: TransferItem, tx_id: String) -> Self {
        Self {
            invoice,
            transfer,
            tx_id,
            tx_status: TxStatus::NotFound,
            amount: None,
            uda: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransferItem {
    pub transfer_id: String,
    pub transfer: String,
    pub is_accept: bool,
}

impl TransferItem {
    pub fn with(transfer: &Transfer, is_accept: bool) -> Self {
        Self {
            transfer_id: transfer.consignment_id().to_string(),
            transfer: transfer.to_string(),
            is_accept,
        }
    }
}

#[derive(
    Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize, Clone, Debug, Display, Default,
)]
#[serde(rename_all = "camelCase")]
pub enum TxStatus {
    #[default]
    #[display(inner)]
    #[serde(rename = "not_found")]
    NotFound,
    #[serde(rename = "error")]
    Error(String),
    #[serde(rename = "mempool")]
    Mempool,
    #[serde(rename = "confirmed")]
    Confirmed,
    #[serde(rename = "block")]
    Block { height: u32, timestamp: u64 },
}

#[derive(Serialize, Deserialize, Debug, Clone, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(context(RGBValidationOptions))]
pub struct SignPsbtRequest {
    /// PSBT encoded in Base64
    #[garde(length(min = 0, max = usize::MAX))]
    pub psbt: String,
    /// Descriptors to Sign
    // TODO: Check secure way to validate xpriv desc
    #[garde(skip)]
    pub descriptors: Vec<SecretString>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SignedPsbtResponse {
    /// PSBT is signed?
    pub sign: bool,
    /// PSBT signed
    pub psbt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(context(RGBValidationOptions))]
pub struct PublishPsbtRequest {
    /// PSBT encoded in Base64
    #[garde(length(min = 0, max = usize::MAX))]
    pub psbt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PublishedPsbtResponse {
    /// PSBT is signed?
    pub sign: bool,
    /// Transaction id
    pub txid: String,
}

#[derive(Validate, Serialize, Deserialize, Debug, Clone)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
pub struct WatcherRequest {
    /// The watcher name
    #[garde(ascii)]
    #[garde(length(min = 1, max = 32))]
    pub name: String,
    /// The xpub will be watch
    #[garde(ascii)]
    #[garde(length(min = 1, max = 256))]
    pub btc_xpub: String,
    /// The xpub will be watch
    #[garde(ascii)]
    #[garde(length(min = 1, max = 256))]
    pub rgb_xpub: String,
    /// Recover Mode
    #[garde(skip)]
    pub recover: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatcherResponse {
    /// The watcher name
    pub name: String,
    /// migrate?
    pub network: String,
    /// migrate?
    pub migrate: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WatcherStatusResponse {
    /// The watcher name
    pub name: String,
    /// created?
    pub created: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextAddressResponse {
    pub address: String,
    pub network: String,
    pub keychain: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct NextUtxoResponse {
    pub utxo: Option<String>,
    pub value: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnspentUtxoResponse {
    pub utxos: Vec<UtxoResponse>,
}

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Debug, Clone, Default, Display)]
#[serde(rename_all = "camelCase")]
#[display("{outpoint}:{amount}")]
pub struct UtxoResponse {
    pub outpoint: String,
    pub amount: u64,
    pub status: TxStatus,
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize, Clone, Debug, Display)]
#[serde(rename_all = "camelCase")]
pub enum OrderValue {
    #[display(inner)]
    #[serde(rename = "bitcoin")]
    Bitcoin(u64),
    #[serde(rename = "contract")]
    #[display(inner)]
    Contract(ContractCounterParty),
}

impl OrderValue {
    pub fn bitcoin(amount: u64) -> Self {
        Self::Bitcoin(amount)
    }

    pub fn contract(kit: ContractCounterParty) -> Self {
        Self::Contract(kit)
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize, Clone, Debug, Display)]
#[serde(rename_all = "camelCase")]
#[display("{kit:?}")]
pub struct ContractCounterParty {
    pub kit: ContractKit,
    pub amount: ContractAmount,
}

impl ContractCounterParty {
    pub fn with(kit: ContractKit, amount: ContractAmount) -> Self {
        Self { kit, amount }
    }
}

impl Default for OrderValue {
    fn default() -> Self {
        Self::Bitcoin(0)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
#[display(doc_comments)]
pub struct RgbOfferRequest {
    /// The Offer Strategy
    #[garde(skip)]
    pub strategy: RgbSwap,
    /// Offers
    #[garde(dive)]
    pub offers: Vec<RgbOfferItem>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
#[display("{contract_id}:{contract_amount}@{counter_party}")]
pub struct RgbOfferItem {
    /// The Contract ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub contract_id: String,
    /// Contract Amount
    #[garde(skip)]
    pub contract_amount: String,
    /// CounterParty
    #[garde(skip)]
    pub counter_party: OrderValue,
    /// Offer Expire Date
    #[garde(skip)]
    pub expire_at: Option<i64>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Display)]
#[serde(rename_all = "camelCase")]
#[display("{id}:{amount}@{counter_party}")]
pub struct RgbOfferResponse {
    /// The Order ID
    pub id: String,
    /// Order Status
    pub status: String,
    /// The Contract Kit
    pub contract: ContractKit,
    /// Order Amount
    pub amount: ContractAmount,
    /// Counterparty
    pub counter_party: OrderValue,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
#[display("bid:{offer_id}:{asset_price}")]
pub struct RgbBidRequest {
    /// The Offer ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: String,
    /// Asset Amount
    #[garde(skip)]
    pub asset_price: OrderValue,
    /// L1 Fee
    #[garde(dive)]
    pub fee: ChainFee,
    /// Optional Bundle ID (For Auctions)
    #[garde(skip)]
    pub bundle_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RgbBidResponse {
    /// The Order ID
    pub id: String,
    /// The Order ID
    pub offer_id: String,
    /// Order Status
    pub status: String,
    /// The Asset Kit
    pub asset: AssetKit,
    /// Order Amount
    pub amount: ContractAmount,
    /// Counterparty
    pub counter_party: OrderValue,
    /// Anchor Bitcoin transaction id (non-optional; must be present on successful bid)
    pub txid: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
#[display("{bundle_id}")]
pub struct RgbAuctionCloseRequest {
    /// Bundle ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub bundle_id: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
#[display("{offer_id}:{bid_id}@{shared_key}")]
pub struct RgbSwapRequest {
    /// The Offer ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub offer_id: String,
    /// The Bid ID
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub bid_id: String,
    /// The Share Key
    #[garde(ascii)]
    #[garde(length(min = 0, max = 100))]
    pub shared_key: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{consig_id}")]
pub struct RgbSwapResponse {
    /// Consignment ID (in base58)
    pub consig_id: String,
    /// Consignment (in armored)
    pub consig: String,
    /// L1 Transaction ID
    pub txid: String,
    /// PSBT File (in base64)
    pub psbt: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
#[display("{offers:?}")]
pub struct RgbCancelOffersRequest {
    #[garde(skip)]
    pub offers: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default, Validate)]
#[garde(context(RGBValidationOptions))]
#[serde(rename_all = "camelCase")]
#[display("{bids:?}")]
pub struct RgbCancelBidsRequest {
    #[garde(skip)]
    pub bids: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Display, Default)]
#[serde(rename_all = "camelCase")]
#[display("{bids:?}")]
pub struct RgbSwapCancelResponse {
    pub records: u8,
    pub offers: Vec<String>,
    pub bids: Vec<String>,
}

impl RgbSwapCancelResponse {
    pub fn cancel_offer(&mut self, offer_id: String) -> Self {
        self.records += 1;
        self.offers.push(offer_id);
        self.clone()
    }

    pub fn cancel_bid(&mut self, bid_id: String) -> Self {
        self.records += 1;
        self.bids.push(bid_id);
        self.clone()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Display)]
#[serde(rename_all = "camelCase")]
#[display("{valid}")]
pub struct RgbValidationResponse {
    pub valid: bool,
    pub message: String,
    pub warnings: Vec<String>,
    pub failures: Vec<String>,
}

impl Default for RgbValidationResponse {
    fn default() -> Self {
        Self {
            valid: true,
            warnings: vec![],
            failures: vec![],
            message: String::default(),
        }
    }
}

impl RgbValidationResponse {
    pub fn failure(&mut self, failure: String) -> Self {
        self.valid = false;
        let (key, message) = failure
            .as_str()
            .split_once(':')
            .map(|(a, b)| (Some(a), Some(b)))
            .unwrap_or((Some(&failure), None));

        match (key, message) {
            (Some(key), Some(message)) => {
                self.failures.push(key.trim().to_string());
                self.message = message.trim().to_string();
            }
            (Some(key), None) => {
                self.failures.push(key.trim().to_string());
            }
            _ => {}
        }
        self.to_owned()
    }

    pub fn warning(&mut self, warning: String) -> Self {
        self.valid = false;
        let (key, message) = warning
            .as_str()
            .split_once(':')
            .map(|(a, b)| (Some(a), Some(b)))
            .unwrap_or((Some(&warning), None));

        match (key, message) {
            (Some(key), Some(_)) => {
                self.warnings.push(key.to_string());
                // self.message = message.trim().to_string();
            }
            (Some(key), None) => {
                self.warnings.push(key.trim().to_string());
            }
            _ => {}
        }
        self.to_owned()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatcherWalletResponse {
    pub next_utxo: Option<String>,
    pub next_utxo_value: Option<u64>,
    pub next_address: Option<String>,
    pub list_contracts: Vec<ContractResponse>,
    pub transfers: Vec<RgbTransferItem>,
    pub invoices: Vec<RgbInvoiceStatusResponse>,
    pub offers: Vec<RgbOfferResponse>,
    pub bids: Vec<RgbBidResponse>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AtomicStatsResponse {
    pub contracts: Vec<String>,
    pub addresses: Vec<String>,
    pub slots_free: Vec<UtxoKit>,
    pub slots_locked: Vec<UtxoKit>,
}
