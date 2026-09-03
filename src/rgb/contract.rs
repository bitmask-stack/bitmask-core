use crate::rgb::models::{AssetType, RgbAccount};
use crate::rgb::prefetch::{BitmaskResolver, ResolverError};
use crate::structs::{
    ContractAllocation, ContractResponse, ContractValue, IssueMedia, UDAItem, UDAValue,
};

use amplify::hex::FromHex;
use bpwallet::{Outpoint, Psbt, Terminal, Txid};
use rgb::ContractId;
use rgbinvoice::Allocation as InvoiceAllocation;
use rgbstd::containers::Transfer;
use rgbstd::contract::{ContractData, ContractError, FilterIncludeAll, SchemaWrapper};
use rgbstd::persistence::{ContractStateRead, Stock};
use schemata::{NiaWrapper, UdaWrapper};

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ExportContractError {
    /// The contract or iface {0} is not supported
    NoSupport(String),
    /// The contract {0} is not found
    NoContrat(String),
    /// The iface {0} is not found
    NoIface(String),
    /// The transfer terminal {0} is not found
    NoTerminal(String),
    /// The transfer {0} presents inconsistency
    WrongTransfer(String),
    /// The contract {0} presents global inconsistency ({1})
    WrongGlobal(String, ContractError),
    /// The contract {0} presents stash inconsistency ({1})
    WrongStash(String, String),
    /// The contract {0} presents strict inconsistency ({1})
    WrongStrict(String, String),
    /// The contract {0} cannot be converted to {1}
    ContractFormat(String, String),
    /// outpoint {0} status retrieve fail
    UtxoStatusFail(String),
    /// resolver error. {0}
    ResolverError(#[from] ResolverError),
}

#[derive(Clone)]
/// Options used in issuing asset operation
pub struct RetrieveContractOptions {
    /// Feature-flag to enable export hidden contracts
    pub hidden_contracts: bool,
    /// Feature-flag to enable sync data to carbonado server
    pub sync: bool,
}

impl Default for RetrieveContractOptions {
    fn default() -> Self {
        Self {
            hidden_contracts: true,
            sync: true,
        }
    }
}

pub async fn get_contract(
    contract_id: ContractId,
    rgb_stock: &mut Stock,
    rgb_account: &mut RgbAccount,
    rgb_resolver: &mut BitmaskResolver,
) -> Result<ContractResponse, ExportContractError> {
    let contract_exported = rgb_stock
        .export_contract(contract_id)
        .map_err(|err| ExportContractError::NoContrat(err.to_string()))?;

    let contract_data = rgb_stock
        .contract_data(contract_id)
        .map_err(|err| ExportContractError::NoContrat(err.to_string()))?;

    let schema_id = contract_exported.schema.schema_id();
    let asset_type = AssetType::from_schema(schema_id)
        .ok_or_else(|| ExportContractError::NoSupport(schema_id.to_string()))?;
    let mut resp = match asset_type {
        AssetType::RGB20 => fungible_info(contract_data, rgb_account, rgb_resolver).await,
        AssetType::RGB21 => uda_info(contract_data, rgb_account, rgb_resolver).await,
        _ => Err(ExportContractError::NoSupport(asset_type.to_string())),
    }?;

    let contract_armored = format!("{contract_exported}");
    resp.contract = contract_armored;

    Ok(resp)
}

#[allow(clippy::field_reassign_with_default)] // allocations filled from contract state after Default
async fn fungible_info<S>(
    contract_data: ContractData<S>,
    rgb_account: &mut RgbAccount,
    rgb_resolver: &mut BitmaskResolver,
) -> Result<ContractResponse, ExportContractError>
where
    S: ContractStateRead,
{
    let client = rgb_resolver.clone().explorer()?;

    let contract_info = contract_data.info.clone();
    let fungible_info = NiaWrapper::with(contract_data);
    let mut resp = ContractResponse::default();

    resp.contract_id = contract_info.id.to_string();
    resp.iface = "RGB20".to_string();

    let asset_spec = fungible_info.spec();

    resp.name = asset_spec.name.to_string();
    resp.ticker = asset_spec.ticker.to_string();
    resp.description = asset_spec.details.map_or(String::new(), |f| f.to_string());

    resp.supply = fungible_info.total_issued_supply().value();
    resp.precision = asset_spec.precision.decimals();
    resp.created = contract_info.issued_at.timestamp();

    // Derive seller host outpoints from persisted offers + swap supplements
    // Also build a mapping outpoint -> terminal to aid pre-index fallback when pending_host entry is missing.
    let (host_outpoints, _host_term_by_outpoint): (
        std::collections::HashSet<String>,
        std::collections::BTreeMap<String, Terminal>,
    ) = {
        let mut set = std::collections::HashSet::new();
        let mut map = std::collections::BTreeMap::<String, Terminal>::new();
        for off in rgb_account.offers_by(FilterIncludeAll) {
            if let Some(sw) = off.swap()
                && let Ok(swap_psbt) = Psbt::from_base64(&sw.psbt)
            {
                let txid_s = swap_psbt.txid().to_string();
                let mut host_vout_index: Option<usize> = None;
                let mut host_term_opt: Option<Terminal> = None;

                if let Some(suppl) = off.suppl() {
                    // Prefer strong scriptPubKey binding if present
                    if let Some(spk_hex) = &suppl.seller_host_spk
                        && let Ok(spk_bytes) = Vec::<u8>::from_hex(spk_hex)
                        && let Some((vi, _)) = swap_psbt
                            .to_unsigned_tx()
                            .outputs
                            .iter()
                            .enumerate()
                            .find(|(_, o)| o.script_pubkey.as_slice() == spk_bytes.as_slice())
                    {
                        host_vout_index = Some(vi);
                    }
                    // Adopt persisted host terminal if available
                    if let Some(host) = suppl.seller_host_terminal.clone() {
                        host_term_opt = Some(Terminal::new(host.keychain, host.index));
                    }
                    // Fall back to terminal derivation match
                    if host_vout_index.is_none()
                        && let Some(host) = suppl.seller_host_terminal.clone()
                    {
                        for (i, o) in swap_psbt.outputs().enumerate() {
                            if let Some(der) = o.terminal_derivation()
                                && der.keychain == host.keychain
                                && der.index == host.index
                            {
                                host_vout_index = Some(i);
                                if host_term_opt.is_none() {
                                    host_term_opt = Some(Terminal::new(der.keychain, der.index));
                                }
                                break;
                            }
                        }
                    }
                }

                if let Some(vi) = host_vout_index {
                    let key = format!("{}:{}", txid_s, vi);
                    set.insert(key.clone());
                    if let Some(term) = host_term_opt {
                        map.insert(key, term);
                    }
                }
            }
        }
        // Telemetry: dump small samples to confirm visibility at export time
        {
            let ph = rgb_account.pending_hosts();
            let ph_keys: Vec<_> = ph.keys().take(5).cloned().collect();
            let commits = rgb_account.tapret_commits();
            let commit_terms: Vec<_> = commits.keys().take(5).map(|t| t.to_string()).collect();
            let host_keys: Vec<_> = map.keys().take(5).cloned().collect();
            crate::debug!(format!(
                "export.fallback.telemetry pending_hosts_count={} sample={:?} tapret_terms_count={} sample={:?} host_term_by_outpoint_count={} sample={:?}",
                ph.len(),
                ph_keys,
                commits.len(),
                commit_terms,
                map.len(),
                host_keys
            ));
        }
        (set, map)
    };

    let mut contract_states = vec![];
    for allocation in fungible_info.allocations(FilterIncludeAll) {
        let (mine, spend, derivation) = if let Ok((wallet_utxo, _)) = rgb_account
            .wallet
            .outpoint_by(crate::rgb::compat::outpoint_bp_from_output_seal(
                allocation.seal,
            )) {
            // TODO: implement resolver cached-version
            let Outpoint { txid, vout, .. } = wallet_utxo.outpoint;
            let spend = if let Ok(Some(utxo_status)) = client
                .output_status(&txid, vout.into_u32().into())
                .await
                .map_err(|err| ExportContractError::ResolverError(ResolverError::Error(err)))
            {
                utxo_status.spent
            } else {
                false
            };

            (true, spend, wallet_utxo.terminal.to_string())
        } else {
            let outpoint: Outpoint =
                crate::rgb::compat::outpoint_bp_from_output_seal(allocation.seal);
            let spend = if let Ok(Some(utxo_status)) = client
                .output_status(&outpoint.txid, outpoint.vout.into_u32().into())
                .await
                .map_err(|err| ExportContractError::ResolverError(ResolverError::Error(err)))
            {
                utxo_status.spent
            } else {
                false
            };
            // Pre-index fallback: accept pending_host or host_outpoints without requiring tapret (UX-first)
            let out_s = outpoint.to_string();
            if let Some(term) = rgb_account.is_pending_host_outpoint(&out_s) {
                crate::debug!(format!(
                    "export.rgb20.pending_host_match terminal={} outpoint={}",
                    term, out_s
                ));
                crate::debug!(format!(
                    "export.rgb20.derivation_fallback isMine=true(no-tapret-check) terminal={} outpoint={}",
                    term, out_s
                ));
                (true, spend, term.to_string())
            } else if host_outpoints.contains(&out_s) {
                crate::debug!(format!(
                    "export.rgb20.hostmap_match(outpoints) outpoint={}",
                    out_s
                ));
                crate::debug!(format!(
                    "export.rgb20.derivation_fallback isMine=true(hostmap no-tapret-check) outpoint={}",
                    out_s
                ));
                (true, spend, String::new())
            } else {
                crate::debug!(format!(
                    "export.rgb20.pending_fallback outpoint={} mine=false spent={}",
                    out_s, spend
                ));
                (false, spend, String::new())
            }
        };

        contract_states.push(ContractAllocation {
            utxo: allocation.seal.to_string(),
            value: ContractValue::Value(allocation.state.value()),
            derivation,
            is_mine: mine,
            is_spent: spend,
        });
    }

    let balance: u64 = contract_states
        .iter()
        .filter(|cs| cs.is_mine && !cs.is_spent && matches!(cs.value, ContractValue::Value(_)))
        .map(|x| match x.value {
            ContractValue::Value(amount) => amount,
            _ => 0,
        })
        .sum::<u64>();

    resp.balance = ContractValue::Value(balance);
    resp.allocations = contract_states;

    Ok(resp)
}

#[allow(clippy::field_reassign_with_default)] // allocations filled from UDA state after Default
async fn uda_info<S>(
    contract_data: ContractData<S>,
    rgb_account: &mut RgbAccount,
    rgb_resolver: &mut BitmaskResolver,
) -> Result<ContractResponse, ExportContractError>
where
    S: ContractStateRead,
{
    let client = rgb_resolver.clone().explorer()?;

    let contract_info = contract_data.info.clone();
    let uda = UdaWrapper::with(contract_data);

    let spec = uda.spec();
    let terms = uda.contract_terms();
    let token_data = uda.token_data();

    let mut resp = ContractResponse::default();

    resp.contract_id = contract_info.id.to_string();
    resp.iface = "RGB21".to_string();

    resp.name = spec.name.to_string();
    resp.ticker = spec.ticker.to_string();
    resp.description = terms.text.to_string();
    resp.created = contract_info.issued_at.timestamp();

    // Fixed data
    resp.supply = 1;
    resp.precision = 0;

    // Derive seller host outpoints + outpoint -> terminal map (RGB21)
    let (host_outpoints, _host_term_by_outpoint): (
        std::collections::HashSet<String>,
        std::collections::BTreeMap<String, Terminal>,
    ) = {
        let mut set = std::collections::HashSet::new();
        let mut map = std::collections::BTreeMap::<String, Terminal>::new();
        for off in rgb_account.offers_by(FilterIncludeAll) {
            if let Some(sw) = off.swap()
                && let Ok(swap_psbt) = Psbt::from_base64(&sw.psbt)
            {
                let txid_s = swap_psbt.txid().to_string();
                let mut host_vout_index: Option<usize> = None;
                let mut host_term_opt: Option<Terminal> = None;

                if let Some(suppl) = off.suppl() {
                    // Prefer strong scriptPubKey binding if present
                    if let Some(spk_hex) = &suppl.seller_host_spk
                        && let Ok(spk_bytes) = Vec::<u8>::from_hex(spk_hex)
                        && let Some((vi, _)) = swap_psbt
                            .to_unsigned_tx()
                            .outputs
                            .iter()
                            .enumerate()
                            .find(|(_, o)| o.script_pubkey.as_slice() == spk_bytes.as_slice())
                    {
                        host_vout_index = Some(vi);
                    }
                    // Adopt persisted host terminal if available
                    if let Some(host) = off
                        .suppl()
                        .as_ref()
                        .and_then(|s| s.seller_host_terminal.clone())
                    {
                        host_term_opt = Some(Terminal::new(host.keychain, host.index));
                    }
                    // Fall back to terminal derivation (keychain/index) match
                    if host_vout_index.is_none()
                        && let Some(host) = off
                            .suppl()
                            .as_ref()
                            .and_then(|s| s.seller_host_terminal.clone())
                    {
                        for (i, o) in swap_psbt.outputs().enumerate() {
                            if let Some(der) = o.terminal_derivation()
                                && der.keychain == host.keychain
                                && der.index == host.index
                            {
                                host_vout_index = Some(i);
                                if host_term_opt.is_none() {
                                    host_term_opt = Some(Terminal::new(der.keychain, der.index));
                                }
                                break;
                            }
                        }
                    }
                }

                if let Some(vi) = host_vout_index {
                    let key = format!("{}:{}", txid_s, vi);
                    set.insert(key.clone());
                    if let Some(term) = host_term_opt {
                        map.insert(key, term);
                    }
                }
            }
        }
        // Telemetry
        {
            let ph = rgb_account.pending_hosts();
            let ph_keys: Vec<_> = ph.keys().take(5).cloned().collect();
            let commits = rgb_account.tapret_commits();
            let commit_terms: Vec<_> = commits.keys().take(5).map(|t| t.to_string()).collect();
            let host_keys: Vec<_> = map.keys().take(5).cloned().collect();
            crate::debug!(format!(
                "export.fallback.telemetry RGB21 pending_hosts_count={} sample={:?} tapret_terms_count={} sample={:?} host_term_by_outpoint_count={} sample={:?}",
                ph.len(),
                ph_keys,
                commits.len(),
                commit_terms,
                map.len(),
                host_keys
            ));
        }
        (set, map)
    };
    let mut contract_states = vec![];
    for allocation in uda.allocations(FilterIncludeAll).collect::<Vec<_>>() {
        let (mine, spend, derivation) = if let Ok((wallet_utxo, _)) = rgb_account
            .wallet
            .outpoint_by(crate::rgb::compat::outpoint_bp_from_output_seal(
                allocation.seal,
            )) {
            // TODO: implement resolver cached-version
            let Outpoint { txid, vout, .. } = wallet_utxo.outpoint;
            let spend = if let Ok(Some(utxo_status)) = client
                .output_status(&txid, vout.into_u32().into())
                .await
                .map_err(|err| ExportContractError::ResolverError(ResolverError::Error(err)))
            {
                utxo_status.spent
            } else {
                false
            };

            (true, spend, wallet_utxo.terminal.to_string())
        } else {
            let outpoint: Outpoint =
                crate::rgb::compat::outpoint_bp_from_output_seal(allocation.seal);
            let spend = if let Ok(Some(utxo_status)) = client
                .output_status(&outpoint.txid, outpoint.vout.into_u32().into())
                .await
                .map_err(|err| ExportContractError::ResolverError(ResolverError::Error(err)))
            {
                utxo_status.spent
            } else {
                false
            };

            // Pre-index fallback: accept pending_host or host_outpoints without requiring tapret (UX-first)
            let out_s = outpoint.to_string();
            if let Some(term) = rgb_account.is_pending_host_outpoint(&out_s) {
                crate::debug!(format!(
                    "export.rgb21.pending_host_match terminal={} outpoint={}",
                    term, out_s
                ));
                crate::debug!(format!(
                    "export.rgb21.derivation_fallback isMine=true(no-tapret-check) terminal={} outpoint={}",
                    term, out_s
                ));
                (true, spend, term.to_string())
            } else if host_outpoints.contains(&out_s) {
                crate::debug!(format!(
                    "export.rgb21.hostmap_match(outpoints) outpoint={}",
                    out_s
                ));
                crate::debug!(format!(
                    "export.rgb21.derivation_fallback isMine=true(hostmap no-tapret-check) outpoint={}",
                    out_s
                ));
                (true, spend, String::new())
            } else {
                crate::debug!(format!(
                    "export.rgb21.pending_fallback outpoint={} mine=false spent={}",
                    out_s, spend
                ));
                (false, spend, String::new())
            }
        };

        contract_states.push(ContractAllocation {
            utxo: allocation.seal.to_string(),
            value: ContractValue::UDA(InvoiceAllocation::from(allocation.state).into()),
            derivation,
            is_mine: mine,
            is_spent: spend,
        });
    }

    let balance = contract_states.iter().any(|cs| cs.is_mine && !cs.is_spent);
    resp.balance = ContractValue::UDA(UDAValue::with(token_data.index.into(), balance.into()));
    resp.allocations = contract_states;

    let mut uda_media = IssueMedia::default();
    if let Some(preview) = token_data.preview {
        uda_media.preview = Some(preview.into())
    }
    if let Some(media) = token_data.media {
        uda_media.media = Some(media.into())
    }
    for (_, attach) in token_data.attachments {
        uda_media.attachments.push(attach.into())
    }

    resp.meta = Some(UDAItem::with(token_data.index.into(), uda_media));

    Ok(resp)
}

#[allow(clippy::result_large_err)]
pub fn extract_transfer(transfer: &Transfer) -> Result<(Txid, &Transfer), ExportContractError> {
    let transfer_id = transfer.consignment_id();
    for (bundle_id, _) in transfer.terminals.clone() {
        if let Some(witness_id) = transfer
            .bundles
            .iter()
            .find(|x| x.bundle.bundle_id() == bundle_id)
            .map(|x| x.pub_witness.clone())
        {
            return Ok((
                crate::rgb::compat::txid_bitcoin_to_bp(witness_id.txid()),
                transfer,
            ));
        }
    }
    Err(ExportContractError::WrongTransfer(transfer_id.to_string()))
}
