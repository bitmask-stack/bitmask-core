#![allow(clippy::result_large_err)] // RgbAccountError / TransferError are product Results; do not reshape
use crate::debug;
use crate::rgb::data::RGBStock;
use crate::rgb::models::ContractValue;
use crate::rgb::models::{
    AssetType, ContractAmount, Direction, RgbAccount, RgbInvoiceInfo, RgbOut, RgbTransfer,
};
use crate::rgb::{AcceptError, InvoiceError, RgbAccountError, TransferError};
use crate::structs::{
    AcceptRequest, AcceptResponse, ChainFee, RgbInvoiceRequest, RgbInvoiceValue, RgbSelection,
    RgbTransferItem, RgbTransferRequest, SecretString, TransferIn, TransferItem, TransferOut,
    UDAValue,
};
use std::str::FromStr;

use crate::bitcoin::fee_rate_from_sat_vb;
use crate::rgb::compat::{rgb_keychain_contains, rgb_tapret_keychain_u8};
use crate::rgb::contract::extract_transfer;
use crate::rgb::ext::{PayExt, WalletExt};
use crate::rgb::mapper::AssetSchema;
use crate::rgb::proxy::get_consignment_ex;
use bpwallet::{Address, Keychain, Outpoint, Psbt, PsbtVer, Sats};
use nonasync::persistence::CloneNoPersistence;
use rgb::TransferParams;
use rgbcore::validation::ResolveWitness;
use rgbcore::{ContractId, GraphSeal};
use rgbinvoice::{Beneficiary, Pay2Vout, Precision, RgbInvoice, RgbInvoiceBuilder, XChainNet};
use rgbstd::containers::{ConsignmentExt, Transfer};
use rgbstd::contract::FilterIncludeAll;
use seals::txout::CloseMethod;

#[derive(Clone)]
/// Options used in invoice asset operation
pub struct RgbInvoiceOptions {
    /// Define the default close method for issue assets
    pub method: CloseMethod,
    /// Feature-flag to enable sync data to carbonado server
    pub sync: bool,
    /// Define the default close method for issue assets
    pub keychain: Option<Keychain>,
}

impl Default for RgbInvoiceOptions {
    fn default() -> Self {
        Self {
            sync: true,
            method: CloseMethod::TapretFirst,
            keychain: None,
        }
    }
}

impl RgbInvoiceOptions {
    pub fn keychain(keychain: Keychain) -> Self {
        Self {
            sync: true,
            method: CloseMethod::TapretFirst,
            keychain: Some(keychain),
        }
    }
}

#[allow(clippy::result_large_err)]
pub fn create_rgb_invoice(
    request: RgbInvoiceRequest,
    _options: RgbInvoiceOptions,
    rgb_stock: &mut RGBStock,
    rgb_account: &mut RgbAccount,
) -> Result<RgbInvoiceInfo, InvoiceError> {
    let RgbInvoiceRequest {
        contract_id,
        amount,
        seal: selection,
        expire_at,
        ..
    } = request;

    let mut precision = 0;
    let (address, outpoint) = match selection {
        RgbSelection::NextUTXO => {
            let avaliable_utxo = rgb_account
                .wallet
                .coinselect(Sats::ZERO, |utxo| {
                    rgb_keychain_contains(utxo.terminal.keychain)
                })
                .next();
            (None, avaliable_utxo)
        }
        RgbSelection::NextAddress => {
            let avaliable_addr = rgb_account
                .wallet
                .addresses(Keychain::with(rgb_tapret_keychain_u8()))
                .next()
                .ok_or(InvoiceError::NoUtxo)?
                .addr;

            (Some(avaliable_addr), None)
        }
        RgbSelection::UTXO(utxo) => (None, Some(Outpoint::from_str(&utxo)?)),
        RgbSelection::Address(address) => (Some(Address::from_str(&address)?), None),
    };

    let network = rgb_account.wallet.network();
    let beneficiary = match (address, outpoint) {
        (Some(addr), None) => {
            debug!(format!(
                "register address {} as secret seal",
                addr.to_string()
            ));
            Beneficiary::WitnessVout(
                Pay2Vout::new(
                    crate::rgb::compat::address_payload_bitcoin_from_script_pubkey(
                        &addr.script_pubkey(),
                    ),
                ),
                none!(),
            )
        }
        (_, Some(outpoint)) => {
            debug!(format!(
                "register outpoint {} as secret seal",
                outpoint.to_string()
            ));
            let seal = GraphSeal::new_random(
                crate::rgb::compat::txid_bp_to_bitcoin(outpoint.txid),
                crate::rgb::compat::vout_bp_to_rgb(outpoint.vout),
            );
            rgb_stock
                .store_secret_seal(seal)
                .map_err(|op| InvoiceError::Stock(op.to_string()))?;

            let secret_seal = seal.to_secret_seal();
            Beneficiary::BlindedSeal(secret_seal)
        }
        _ => return Err(InvoiceError::WrongSelection),
    };

    let mut builder = RgbInvoiceBuilder::new(XChainNet::bitcoin(
        crate::rgb::compat::network_bp_to_bitcoin(network),
        beneficiary,
    ));
    if let Some(contract_id) = contract_id.clone() {
        let contract_id = ContractId::from_str(&contract_id).map_err(|_| {
            InvoiceError::Validation(bmap! { "contract_id".to_string() => "parameter is invalid (parse error)".to_string() })
        })?;
        builder = builder.set_contract(contract_id);
    };

    if let Some(expire_at) = expire_at {
        builder = builder.set_expiry_timestamp(expire_at);
    };

    match amount {
        RgbInvoiceValue::Value(value) => {
            let coin_amount = ContractAmount::convert(value);
            let precision = Precision::try_from(coin_amount.precision).map_err(|_| {
                InvoiceError::Validation(
                    bmap! { "invoice_amount".to_string() => "invalid precision".to_string() },
                )
            })?;
            builder = builder
                .set_amount(coin_amount.int, coin_amount.fract, precision)
                .map_err(|_| {
                    InvoiceError::Validation(
                        bmap! { "invoice_amount".to_string() => "invalid amount".to_string() },
                    )
                })?
        }
        RgbInvoiceValue::UDA(allocation) => {
            let UDAValue {
                token_index,
                fraction,
            } = allocation;

            builder = builder.set_allocation(token_index, fraction).map_err(|_| {
                InvoiceError::Validation(
                    bmap! { "invoice_amount".to_string() => "invalid allocation".to_string() },
                )
            })?
        }
    };

    let secret_seal = if let Some(outpoint) = outpoint {
        outpoint.to_string()
    } else if let Some(address) = address {
        address.to_string()
    } else {
        return Err(InvoiceError::NoUtxo);
    };

    let rgb_invoice = builder.finish();

    if let Some(contract_id) = contract_id.clone() {
        let contract_id = ContractId::from_str(&contract_id).map_err(|_| {
            InvoiceError::Validation(bmap! { "contract_id".to_string() => "parameter is invalid (parse error)".to_string() })
        })?;

        if let Some(contract_info) = rgb_account.contract_by(contract_id) {
            precision = *contract_info.precision()
        }
    };

    let rgb_info = RgbInvoiceInfo::new(rgb_invoice, secret_seal, precision);

    Ok(rgb_info)
}

#[derive(Clone)]
/// Options used in transfer asset operation
pub struct RgbTransferOptions {
    /// Define the default close method for transfer assets
    pub method: CloseMethod,
    /// Defines which version of the PSBT in response
    pub psbt_version: PsbtVer,
    /// Feature flag to enable publish consig information in proxy
    pub proxy: bool,
    /// Feature flag to enable PSBT signing in response
    pub sign: bool,
    /// Feature-flag to enable sync data to carbonado server
    pub sync: bool,
    /// Private key(s) for PSBT signing (flag sign=true)
    pub private_keys: Vec<SecretString>,
}

impl RgbTransferOptions {
    pub fn broadcast(private_keys: Vec<SecretString>) -> Self {
        Self {
            sign: true,
            proxy: true,
            private_keys,
            ..Default::default()
        }
    }
}

impl Default for RgbTransferOptions {
    fn default() -> Self {
        Self {
            psbt_version: PsbtVer::V0,
            method: CloseMethod::TapretFirst,
            sync: true,
            proxy: true,
            sign: false,
            private_keys: vec![],
        }
    }
}

#[derive(Clone)]
pub struct RgbTransferResult {
    pub psbt: Psbt,
    pub consig: Transfer,
}

pub async fn create_rgb_transfer(
    request: RgbTransferRequest,
    options: RgbTransferOptions,
    rgb_stock: &mut RGBStock,
    rgb_account: &mut RgbAccount,
) -> Result<RgbTransferResult, TransferError> {
    let RgbTransferRequest {
        contract_id,
        iface,
        invoice,
        chain_fee,
        sender_amount,
        ..
    } = request;

    let mut rgb_invoice =
        RgbInvoice::from_str(&invoice).map_err(|err| TransferError::Invoice(err.to_string()))?;

    // For blinded invoices, inject sender_amount into the invoice only for RGB20.
    // For RGB21 blinded, we never inject fungible amount; the count is conveyed to the proxy.
    let is_zero_amount = matches!(
        &rgb_invoice.assignment_state,
        Some(rgbinvoice::InvoiceState::Amount(a)) if a.value() == 0
    );
    debug!(format!("is_zero_amount {is_zero_amount}"));
    if (rgb_invoice.assignment_state.is_none() || is_zero_amount)
        && let Some(sa) = sender_amount.clone()
    {
        let is_blinded = matches!(
            rgb_invoice.beneficiary.into_inner(),
            Beneficiary::BlindedSeal(_)
        );
        if !is_blinded {
            return Err(TransferError::Validation(
                bmap! {"sender_amount".to_string() => "only allowed for blinded invoices".to_string()},
            ));
        }

        let asset_type = AssetType::from_str(&iface).unwrap();
        debug!(format!("asset_type {asset_type}"));
        match (asset_type, sa) {
            // Policy: RGB21 blinded => always inject 1-of-1 UDA for now, regardless of sender_amount variant.
            // TODO(bitmask/rgb21): support selecting specific token index and custom fraction from sender_amount.
            (AssetType::RGB21, _) => {
                debug!(
                    "AssetType::RGB21, forcing 1-of-1 UDA allocation (tokenIndex=1, fraction=1)"
                );
                let alloc = rgbinvoice::Allocation::with(1, 1);
                rgb_invoice.assignment_state = Some(rgbinvoice::InvoiceState::Data(
                    rgbinvoice::NonFungible::FractionedToken(alloc),
                ));
            }
            // RGB20: preserve previous behavior (inject fungible amount)
            (_, RgbInvoiceValue::Value(v)) => {
                let a: rgbinvoice::Amount = ContractAmount::convert(v).to_value().into();
                debug!(format!("asset_aaaa {a}"));
                rgb_invoice.assignment_state = Some(rgbinvoice::InvoiceState::Amount(a));
            }
            // Disallow UDA for non-RGB21 blinded invoices
            (_, RgbInvoiceValue::UDA(_)) => {
                debug!("RgbInvoiceValue::UDA(_) not allowed for non-RGB21");
                return Err(TransferError::Validation(
                    bmap! {"sender_amount".to_string() => "UDA not allowed for blinded invoices except RGB21".to_string()},
                ));
            }
        }
    }

    let transfer_contract_id = ContractId::from_str(&contract_id)
        .map_err(|err| TransferError::Invoice(err.to_string()))?;
    if rgb_invoice.contract.is_none() {
        rgb_invoice.contract = Some(transfer_contract_id);
    }

    let mut rgb_wallet = rgb_account.wallet.clone_no_persistence();
    let fee = match chain_fee {
        ChainFee::Value(value) => value,
        ChainFee::FeeRate(fee_rate) => {
            let params = TransferParams::with(0, 0);
            let fee_rate = fee_rate_from_sat_vb(fee_rate);

            let (pre_psbt, _) = rgb_wallet
                .partial_psbt(
                    rgb_stock,
                    &rgb_invoice,
                    crate::rgb::compat::close_method_from_dbc(options.method),
                    params,
                )
                .map_err(|op| TransferError::FeeEstimate(op.to_string()))?;

            rgb_wallet
                .fee_estimate(pre_psbt, fee_rate)
                .map_err(|op| TransferError::FeeEstimate(op.to_string()))?
        }
        _ => {
            return Err(TransferError::Validation(
                bmap! {"fee".to_string() => "unsupported fee".to_string()},
            ));
        }
    };

    let params = TransferParams::with(fee, 0);

    let (psbt, _, consig) = rgb_account
        .wallet
        .make_pay(rgb_stock, &rgb_invoice, params)
        .map_err(|op| TransferError::Pay(op.to_string()))?;

    Ok(RgbTransferResult { psbt, consig })
}

#[derive(Clone)]
pub struct RgbAcceptOptions {
    /// Feature-flag to enable sync data to carbonado server
    sync: bool,
}

impl Default for RgbAcceptOptions {
    fn default() -> Self {
        Self { sync: true }
    }
}

impl RgbAcceptOptions {
    pub fn sync(self) -> bool {
        self.sync
    }
}

#[allow(clippy::result_large_err)]
pub fn accept_rgb_transfer<T>(
    request: AcceptRequest,
    rgb_stock: &mut RGBStock,
    rgb_account: &mut RgbAccount,
    rgb_resolver: &T,
) -> Result<AcceptResponse, AcceptError>
where
    T: ResolveWitness + std::fmt::Debug,
{
    let AcceptRequest { consignment, .. } = request;
    let transfer =
        &Transfer::from_str(&consignment).map_err(|op| AcceptError::WrongConsig(op.to_string()))?;

    let chain_net = rgb_account.chain_net();
    let type_system = AssetSchema::from(transfer.schema_id()).types();
    let transition = match transfer.clone().validate(
        rgb_resolver,
        &crate::rgb::compat::validation_config(chain_net, type_system),
    ) {
        Ok(transfer) => transfer,
        Err(status) => return Err(AcceptError::WrongTransfer(status.to_string())),
    };

    // Extract the anchor txid for the UI before we persist anything
    let (txid, _) =
        extract_transfer(transfer).map_err(|err| AcceptError::WrongTransfer(err.to_string()))?;

    rgb_stock
        .accept_transfer(transition, rgb_resolver)
        .map_err(|op| AcceptError::Accept(op.to_string()))?;

    debug!("accepted rgb transfer".to_string());
    let resp = AcceptResponse {
        transfer_id: transfer.consignment_id().to_string(),
        contract_id: transfer.contract_id().to_string(),
        txid: txid.to_string(),
        valid: true,
    };

    Ok(resp)
}

pub async fn list_transfer_items(
    rgb_account: RgbAccount,
) -> Result<Vec<RgbTransferItem>, RgbAccountError> {
    let mut items = vec![];
    for transfer in rgb_account.transfers_by(FilterIncludeAll) {
        let RgbTransfer {
            contract_amount,
            direction,
            chain_info,
            ..
        } = transfer.clone();

        match direction {
            Direction::In(inc) => {
                if let Some(rgb_transfer) = inc.transfer.clone() {
                    let rgb_invoice = inc.invoice.to_string();
                    let rgb_transfer = Transfer::from_str(&rgb_transfer).expect("invalid transfer");
                    let item = TransferItem::with(&rgb_transfer, transfer.is_complete());

                    let (txid, _) = extract_transfer(&rgb_transfer)
                        .map_err(|err| RgbAccountError::WrongTransfer(err.to_string()))?;

                    // Normalize amount for frontend: prefer persisted sender-set amount.
                    // Accept both raw integer (base units) and decimal strings.
                    let mut amount_opt = match &contract_amount {
                        Some(ContractValue::Value(v)) => {
                            debug!(format!("VALUE VALUE 1 {}", v));
                            let precision = rgb_account
                                .contract_by(transfer.contract_id)
                                .map(|ci| *ci.precision())
                                .unwrap_or(0);
                            let value_u64 = match v.parse::<u64>() {
                                Ok(u) => u,
                                Err(_) => {
                                    // Clean legacy strings like "1 aa" by keeping only digits and '.'
                                    let cleaned: String = v
                                        .chars()
                                        .filter(|c| c.is_ascii_digit() || *c == '.')
                                        .collect();
                                    ContractAmount::parse(cleaned, precision).to_value()
                                }
                            };
                            if value_u64 > 0 {
                                Some(ContractAmount::with(value_u64, precision))
                            } else {
                                // Leave None so we can attempt a proxy lookup below.
                                None
                            }
                        }
                        _ => None,
                    };

                    // Prefer persisted UDA if available
                    let mut uda_opt: Option<UDAValue> = match &contract_amount {
                        Some(ContractValue::UDA(alloc)) => Some(alloc.clone()),
                        _ => None,
                    };

                    // If still unknown/zero fungible amount, try to fetch blinded invoice amount from proxy (RGB20 only).
                    if amount_opt.is_none() {
                        let precision = rgb_account
                            .contract_by(transfer.contract_id)
                            .map(|ci| *ci.precision())
                            .unwrap_or(0);
                        if let Ok(parsed_inv) = RgbInvoice::from_str(&rgb_invoice)
                            && let Beneficiary::BlindedSeal(seal) =
                                parsed_inv.beneficiary.into_inner()
                            && let Ok(Some((_armored, sender_amount))) =
                                get_consignment_ex(&seal.to_string()).await
                            && let Some(RgbInvoiceValue::Value(v)) = sender_amount
                        {
                            let ca = ContractAmount::parse(v, precision);
                            if ca.to_value() > 0 {
                                amount_opt = Some(ca);
                            }
                        }
                    }

                    // Listing-time proxy overlay for UDA: if missing and invoice is blinded, ask proxy
                    if uda_opt.is_none()
                        && let Ok(parsed_inv) = RgbInvoice::from_str(&rgb_invoice)
                        && let Beneficiary::BlindedSeal(seal) = parsed_inv.beneficiary.into_inner()
                        && let Ok(Some((_armored, sender_amount))) =
                            get_consignment_ex(&seal.to_string()).await
                        && let Some(RgbInvoiceValue::UDA(uda)) = sender_amount
                    {
                        uda_opt = Some(uda);
                    }

                    let transfer_in = if let Some(transfer_in) = chain_info {
                        let mut t = TransferIn::with(
                            rgb_invoice.to_string(),
                            txid.to_string(),
                            transfer_in.status.into(),
                            Some(item),
                            amount_opt,
                        );
                        t.uda = uda_opt.clone();
                        t
                    } else {
                        let mut tmp = TransferIn::unknown(
                            rgb_invoice.to_string(),
                            txid.to_string(),
                            Some(item),
                        );
                        tmp.amount = amount_opt;
                        tmp.uda = uda_opt.clone();
                        tmp
                    };

                    // Use the real contract id from parsed consignment and derive iface from contract metadata
                    let contract_id_str = rgb_transfer.contract_id().to_string();
                    let iface = if let Ok(cid) = ContractId::from_str(&contract_id_str) {
                        if let Some(info) = rgb_account.contract_by(cid) {
                            info.contract_type().to_string()
                        } else if let Some(ref ca) = contract_amount {
                            if ca.is_uda() {
                                AssetType::RGB21.to_string()
                            } else {
                                AssetType::RGB20.to_string()
                            }
                        } else {
                            "Unknown".to_string()
                        }
                    } else {
                        "Unknown".to_string()
                    };

                    items.push(RgbTransferItem::incoming(
                        &contract_id_str,
                        &iface,
                        transfer_in,
                    ));
                }
            }
            Direction::Out(out) => {
                let RgbOut {
                    transfer: rgb_transfer,
                    invoice,
                    ..
                } = out;
                let rgb_transfer =
                    Transfer::from_str(&rgb_transfer.to_string()).expect("invalid transfer");

                let (txid, parsed_transfer) = extract_transfer(&rgb_transfer)
                    .map_err(|err| RgbAccountError::WrongTransfer(err.to_string()))?;

                // Capture real contract id from the parsed transfer before moving it into item
                let contract_id_str = parsed_transfer.contract_id().to_string();

                // Derive iface from contract metadata (fallback to amount kind)
                let iface = if let Ok(cid) = ContractId::from_str(&contract_id_str) {
                    if let Some(info) = rgb_account.contract_by(cid) {
                        info.contract_type().to_string()
                    } else if let Some(ref ca) = contract_amount {
                        if ca.is_uda() {
                            AssetType::RGB21.to_string()
                        } else {
                            AssetType::RGB20.to_string()
                        }
                    } else {
                        "Unknown".to_string()
                    }
                } else {
                    "Unknown".to_string()
                };

                let item = TransferItem::with(parsed_transfer, transfer.is_complete());

                // Normalize amount for frontend: prefer persisted sender-set amount.
                // Accept both raw integer (base units) and decimal strings.
                let mut amount_opt = match &contract_amount {
                    Some(ContractValue::Value(v)) => {
                        debug!(format!("VALUE VALUE {}", v));
                        let precision = rgb_account
                            .contract_by(transfer.contract_id)
                            .map(|ci| *ci.precision())
                            .unwrap_or(0);
                        let value_u64 = match v.parse::<u64>() {
                            Ok(u) => u,
                            Err(_) => {
                                let cleaned: String = v
                                    .chars()
                                    .filter(|c| c.is_ascii_digit() || *c == '.')
                                    .collect();
                                ContractAmount::parse(cleaned, precision).to_value()
                            }
                        };
                        if value_u64 > 0 {
                            Some(ContractAmount::with(value_u64, precision))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                // Prefer persisted UDA if available
                let mut uda_opt: Option<UDAValue> = match &contract_amount {
                    Some(ContractValue::UDA(alloc)) => Some(alloc.clone()),
                    _ => None,
                };

                // If still unknown for outgoing (fungible), query proxy with our blinded invoice to fetch sender_amount (RGB20 only).
                if amount_opt.is_none() {
                    let precision = rgb_account
                        .contract_by(transfer.contract_id)
                        .map(|ci| *ci.precision())
                        .unwrap_or(0);
                    if let Ok(parsed_inv) = RgbInvoice::from_str(&invoice)
                        && let Beneficiary::BlindedSeal(seal) = parsed_inv.beneficiary.into_inner()
                        && let Ok(Some((_armored, sender_amount))) =
                            get_consignment_ex(&seal.to_string()).await
                        && let Some(RgbInvoiceValue::Value(v)) = sender_amount
                    {
                        let ca = ContractAmount::parse(v, precision);
                        if ca.to_value() > 0 {
                            amount_opt = Some(ca);
                        }
                    }
                }

                // Listing-time proxy overlay for UDA on outgoing as well
                if uda_opt.is_none()
                    && let Ok(parsed_inv) = RgbInvoice::from_str(&invoice)
                    && let Beneficiary::BlindedSeal(seal) = parsed_inv.beneficiary.into_inner()
                    && let Ok(Some((_armored, sender_amount))) =
                        get_consignment_ex(&seal.to_string()).await
                    && let Some(RgbInvoiceValue::UDA(uda)) = sender_amount
                {
                    uda_opt = Some(uda);
                }

                let mut transfer_out = if let Some(chain_info_) = chain_info {
                    // Move chain_info into the if let (no unwrap needed)
                    TransferOut::with(
                        invoice,
                        item,
                        txid.to_string(),
                        chain_info_.status.into(),
                        amount_opt,
                    )
                } else {
                    let mut tmp = TransferOut::unknown(invoice, item, txid.to_string());
                    tmp.amount = amount_opt;
                    tmp
                };
                transfer_out.uda = uda_opt.clone();

                items.push(RgbTransferItem::outcoming(
                    &contract_id_str,
                    &iface,
                    transfer_out,
                ));
            }
        }
    }

    Ok(items)
}
