#![allow(clippy::result_large_err)] // RGB 0.11 error enums are the product Result; do not reshape
pub(crate) mod compat;
pub mod constants;
pub(crate) mod contract;
pub mod data;
pub mod issue;
pub mod mapper;
pub mod models;
pub mod prefetch;
pub mod proxy;
use bpstd::Vout;
pub mod coordinator;
pub mod ext;
mod server;
pub(crate) mod store;
pub mod structs;
pub mod swap;
pub mod transfer;

use crate::bitcoin::{
    decrypt_wallet, encrypt_wallet, hash_password, publish_psbt_file_bp, send_sats,
    sign_and_publish_psbt_file_bp,
};
use crate::bp::models::BtcAccount;
use crate::bp::signer::MainSigner;
use crate::constants::{
    BITCOIN_EXPLORER_API, bp_network_from_str, get_network, get_server_wallet_seed,
};
use crate::rgb::constants::RGB_VERSION;
use crate::rgb::contract::{
    ExportContractError, RetrieveContractOptions, extract_transfer,
    get_contract as get_rgb_contract,
};
use crate::rgb::data::{RGBStock, RGBStockCrossStore};
use crate::rgb::ext::StockExt;
use crate::rgb::issue::{
    IssueContractError, IssueContractOptions, IssueContractParams,
    issue_contract as rgb_issue_contract,
};
use crate::rgb::mapper::AssetSchema;
use crate::rgb::models::{
    AcceptInfo, AssetParseError, AssetType, BidAsset, ChainInfo, ContractAmount, ContractInfo,
    ContractKit, ContractMeta, ContractSwap, CounterParty, Direction, FilterOutpoint, FilterStatus,
    Layer1Meta, OrderBy, OrderId, PublicRgbOffer, RgbAccount, RgbBid, RgbInvoiceItem, RgbOffer,
    RgbTransferInfo, Status, UtxoKit,
};
use crate::rgb::prefetch::{BitmaskResolver, ResolverError, prefetch_transfer};
use crate::rgb::proxy::{
    MediaEncode, MediaItem, MediaPublished, ProxyError, get_consignment_ex, get_media_metadata,
    post_media_metadata, post_media_metadata_list, publish_rgb_transfer,
};
#[cfg(target_arch = "wasm32")]
use crate::rgb::server::{
    backup_rgb_account, backup_rgb_stock, restore_rgb_account, restore_rgb_stock,
};
pub use crate::rgb::server::{clean_public_offers, reset_public_offers};
use crate::rgb::server::{
    get_public_offer, list_fill_public_offers, list_public_offers, match_public_offer,
    remove_public_bid, remove_public_offer, save_public_offers,
};
use crate::rgb::store::{get_account_and_stock, save_account_and_stock};
use crate::rgb::structs::RgbValidation;
use crate::rgb::swap::{
    AnchorShard, BidOptions, NewBidParams, OfferOptions, OrderSupplError, RgbExtraShards, RgbShard,
    SwapErrors, SwapSupplement,
};
use crate::rgb::transfer::{
    RgbAcceptOptions, RgbInvoiceOptions, RgbTransferOptions, RgbTransferResult,
    accept_rgb_transfer, create_rgb_invoice, create_rgb_transfer, list_transfer_items,
};
use crate::structs::{
    AcceptRequest, AcceptResponse, AtomicMintRequest, AtomicMintResponse, AtomicStatsResponse,
    ChainFee, ContractResponse, ContractValue, IssueMedia, IssueRequest, IssueResponse, MediaData,
    OrderValue, PrivateWalletData, PublishPsbtRequest, RgbBidRequest, RgbBidResponse,
    RgbCancelBidsRequest, RgbCancelOffersRequest, RgbInvoiceDecoded, RgbInvoiceRequest,
    RgbInvoiceResponse, RgbInvoiceStatusResponse, RgbInvoiceValue, RgbOfferItem, RgbOfferRequest,
    RgbOfferResponse, RgbSwapCancelResponse, RgbTransferItem, RgbTransferRequest,
    RgbTransferResponse, RgbValidationResponse, SaveTransferRequest, SecretString, SignPsbtRequest,
    TransferIn, TransferItem, UDAItem, WatcherRequest, WatcherWalletResponse,
};
use crate::wallet::data::BPWallet;
use crate::wallet::data::{AccountCrossStore, CrossStoreAsync, CrossStoreError, SyncProvider};
use crate::wallet::interop::XprivBpKey;
use crate::wallet::prefetch::PrefetchSync;
use crate::wallet::server::ServerErrors;
use crate::wallet::watcher::{
    WatcherError, create_watcher, watcher_anchored_utxos, watcher_next_addresses,
    watcher_next_utxo, watcher_prefetch, watcher_unspent_utxo, watcher_unspent_utxos,
};
use crate::{debug, warn};
use amplify::confinement::Confined;
use amplify::confinement::U16;
use amplify::hex::FromHex;
use armor::AsciiArmor;
use bpstd::{AddressNetwork, DeriveScripts, NormalIndex, Outpoint, XprivAccount};
use bpwallet::psbt::{PsbtConstructor, Utxo};
use bpwallet::{Network, Party, Psbt, PsbtVer, Sats, Terminal, TxStatus};
use dbc::tapret::TapretCommitment;
use garde::Validate;
use nonasync::persistence::CloneNoPersistence;
use psrgbt::RgbPsbtExt;
use rgb::TxoSeal;
use rgbcore::ChainNet;
use rgbcore::{ContractId, OutputSeal};
use rgbinvoice::{Allocation, Amount, Beneficiary, InvoiceState, NonFungible, RgbInvoice};
use rgbstd::containers::{ConsignmentExt, Contract, Transfer};
use rgbstd::contract::{AllocatedState, FilterIncludeAll, OwnedAllocation};
use std::collections::BTreeMap;
use std::str::FromStr;
use strict_encoding::{FieldName, StrictDeserialize, TypeName, tn};
use swap::RgbSwap;

#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbError {
    /// Occurs an error in issue step. {0}
    Issue(IssueError),
    /// Occurs an error in invoice step. {0}
    Invoice(InvoiceError),
    /// Occurs an error in transfer step. {0}
    Transfer(TransferError),
    /// Occurs an error in transfer validation step. {0}
    TransferValidation(String),
    /// Occurs an error in accept step. {0}
    Accept(AcceptError),
    /// Occurs an error in wallet retrieve step. {0}
    Account(RgbAccountError),
    /// Occurs an error in check transfers. {0}
    CheckTransfer(RgbAccountError),
    /// Occurs an error in create watcher step. {0}
    CreateWatcher(WatcherError),
    /// Occurs an error in check verify watcher step. {0}
    CheckWatcher(WatcherError),
    /// Occurs an error in watcher next address step. {0}
    CreateNextAddress(WatcherError),
    /// Occurs an error in watcher next utxo step. {0}
    CreateNextUtxo(WatcherError),
    /// Occurs an error in watcher unspent utxos step. {0}
    CreateUnspentUtxo(WatcherError),
    /// Occurs an error in drop watcher step. {0}
    DropWatcher(WatcherError),
    /// Occurs an error in swap step. {0}
    Swap(OrderErrors),
    /// Occurs an error in swap validation validation step. {0}
    SwapValidation(String),
    /// Occurs an error in backup data step. {0}
    Backup(WatcherError),
    /// Occurs an error in restore data step. {0}
    Restore(WatcherError),
    /// Occurs an error in recover data step. {0}
    Recover(WatcherError),
}

#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum IssueError {
    /// request data is wrong. {0:?}
    Validation(BTreeMap<String, String>),
    /// I/O or connectivity error. {0}
    IO(CrossStoreError),
    /// contract proxy fail. {0}
    Proxy(ProxyError),
    /// store retrieve error. {0}
    Store(String),
    /// contract issue fail. {0}
    Issue(IssueContractError),
    /// contract export fail. {0}
    Export(ExportContractError),
    /// account sync error. {0}
    Wallet(String),
    /// unknown network error. {0}
    InvalidNetwork(#[from] bpstd::UnknownNetwork),
    /// invalid issue seal. {0}
    SealParseError(#[from] seals::txout::explicit::ParseError),
    /// prefetch error. {0}
    PrefetchError(#[from] prefetch::PrefetchError),
}

pub async fn issue_contract(
    sk: &str,
    request: IssueRequest,
    options: IssueContractOptions,
) -> Result<IssueResponse, IssueError> {
    if let Err(err) = request.validate() {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(IssueError::Validation(errors));
    }

    let IssueRequest { iface, meta, .. } = request.clone();
    let asset_type = AssetType::from_str(&iface).map_err(|op| {
        let id = "iface".to_string();
        let message = op.to_string();
        IssueError::Validation(bmap! {id => message})
    })?;
    let network = bp_network_from_str(&get_network().await)?;
    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let (mut rgb_account_provider, mut rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| IssueError::Store(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| IssueError::Store(err.to_string()))?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| IssueError::Store(err.to_string()))?;

    let is_uda = asset_type.is_uda();
    let media = if let Some(ref contract_media) = meta {
        if is_uda & options.proxy {
            let published = post_proxy_media(meta).await?;
            Some(published.into())
        } else if is_uda {
            Some(contract_media.clone().into())
        } else {
            None
        }
    } else {
        None
    };

    let params = IssueContractParams::with(request, rgb_account.chain_net(), media);
    let contract = rgb_issue_contract(params, &options, &mut rgb_stock, &mut rgb_resolver)
        .map_err(IssueError::Issue)?;

    let mut contract_exported = get_rgb_contract(
        contract.contract_id(),
        &mut rgb_stock,
        &mut rgb_account,
        &mut rgb_resolver,
    )
    .await
    .map_err(IssueError::Export)?;

    if is_uda
        && options.proxy
        && let Some(media) = contract_exported.meta
    {
        contract_exported.meta = Some(get_proxy_media(media).await.map_err(IssueError::Proxy)?);
    }

    let contract_info = ContractInfo::with(contract_exported.clone());
    rgb_account.add_contract(contract_info);

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| IssueError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| IssueError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| IssueError::Store(err.to_string()))?;
    rgb_stock_provider
        .persist()
        .await
        .map_err(|err| IssueError::Store(err.to_string()))?;

    Ok(contract_exported)
}

async fn get_proxy_media(media: UDAItem) -> Result<UDAItem, ProxyError> {
    let mut contract_item = media.clone();
    let contract_media = media.media;

    let mut meta = IssueMedia {
        preview: contract_media.preview,
        ..Default::default()
    };

    if let Some(media) = contract_media.media.clone()
        && let Some(media_metadata) = get_media_metadata(&media.uri).await?
    {
        meta.media = Some(MediaData::from(media_metadata));
    }

    let attachments = contract_media.attachments.clone();
    for media in attachments {
        if let Some(media_metadata) = get_media_metadata(&media.uri).await? {
            meta.attachments.push(MediaData::from(media_metadata));
        }
    }

    contract_item.media = meta;
    Ok(contract_item)
}

async fn post_proxy_media(request: Option<IssueMedia>) -> Result<MediaPublished, IssueError> {
    let request = request.unwrap_or_default();

    let mut resp = MediaPublished::default();
    if let Some(preview) = request.preview {
        let metadata = post_media_metadata(preview.into(), MediaEncode::Base64)
            .await
            .map_err(IssueError::Proxy)?;

        resp.preview = Some(MediaItem::new(metadata, MediaEncode::Base64));
    } else {
        let id = "preview media".to_string();
        let message = "'preview media' is required".to_string();
        return Err(IssueError::Validation(bmap! {id => message}));
    }

    if let Some(media) = request.media {
        let metadata = post_media_metadata(media.into(), MediaEncode::Sha2)
            .await
            .map_err(IssueError::Proxy)?;

        resp.media = Some(MediaItem::new(metadata, MediaEncode::Sha2));
    } else {
        let id = "media".to_string();
        let message = "'media' is required".to_string();
        return Err(IssueError::Validation(bmap! {id => message}));
    }

    let attachs = post_media_metadata_list(
        request.attachments.into_iter().map(|f| f.into()).collect(),
        MediaEncode::Blake3,
    )
    .await
    .map_err(IssueError::Proxy)?;

    for attach in attachs {
        resp.attachments
            .push(MediaItem::new(attach, MediaEncode::Blake3))
    }

    Ok(resp)
}

trait TransferExt {
    fn is_mined(&self, rgb_account: &RgbAccount) -> bool;
}

impl TransferExt for Transfer {
    // CORRECTED IMPLEMENTATION: This now correctly GETS the chain info.
    fn is_mined(&self, rgb_account: &RgbAccount) -> bool {
        let consignment_id = self.consignment_id();
        // Find the transfer in the account's list
        if let Some(transfer_item) = rgb_account
            .transfers_by(FilterIncludeAll)
            .iter()
            .find(|t| t.transfer_id().is_some_and(|id| id == consignment_id))
        {
            // Check its chain_info field
            if let Some(info) = &transfer_item.chain_info {
                return info.status.is_mined();
            }
        }
        false
    }
}

// Extension trait to add `transfer_is_open` functionality
trait RgbAccountExt {
    fn transfer_is_open(&self, consignment_id: &str) -> bool;
}

impl RgbAccountExt for RgbAccount {
    fn transfer_is_open(&self, consignment_id: &str) -> bool {
        self.transfers_by(FilterIncludeAll)
            .iter()
            .find(|item| {
                if let Some(id) = item.transfer_id() {
                    id.to_string() == consignment_id
                } else {
                    false
                }
            })
            .is_some_and(|item| item.is_open())
    }
}

async fn update_transfer_chain_status(
    rgb_transfer: &Transfer,
    mut rgb_account: RgbAccount,
    rgb_resolver: &mut BitmaskResolver,
) -> Result<RgbAccount, String> {
    let (txid, extracted_transfer) =
        extract_transfer(rgb_transfer).map_err(|err| err.to_string())?;
    let tx_status = rgb_resolver
        .tx_status(&txid)
        .await
        .map_err(|err| err.to_string())?;
    let chain_info = ChainInfo::with(txid, tx_status);
    // Use the dedicated method to update chain info
    rgb_account.chain_info(extracted_transfer.consignment_id(), chain_info);
    debug!(format!(
        "Updated chain status for {}: {:?}",
        extracted_transfer.consignment_id(),
        tx_status
    ));
    Ok(rgb_account)
}

/// Helper: Processes acceptance for a mined, open, incoming transfer.
async fn process_transfer_status_and_accept(
    rgb_transfer: &Transfer,
    rgb_account: &mut RgbAccount,
    rgb_stock: &mut RGBStock,
    rgb_resolver: &mut BitmaskResolver,
) -> Result<(), String> {
    // Pre-conditions are checked in the main loop, but an extra guard is safe.
    if !rgb_transfer.is_mined(rgb_account)
        || !rgb_account.transfer_is_open(&rgb_transfer.consignment_id().to_string())
    {
        return Ok(());
    }

    prefetch_transfer(rgb_transfer, rgb_resolver)
        .await
        .map_err(|e| e.to_string())?;

    let req = AcceptRequest {
        consignment: rgb_transfer.to_string(),
        force: false,
    };

    // Accept the transfer; if it is for a different chain/network than the currently running
    // validator (e.g., "chain-network pair doesn't match"), do not fail verify_transfers.
    // Instead, log and skip gracefully so other transfers can proceed.
    let accept_response = match accept_rgb_transfer(req, rgb_stock, rgb_account, &*rgb_resolver) {
        Ok(resp) => resp,
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("chain-network pair doesn't match") {
                crate::debug!(format!(
                    "verify.swap.accept.skip: consignment chain/network mismatch; skipping acceptance. err={}",
                    msg
                ));
                return Ok(());
            }
            return Err(msg);
        }
    };

    if accept_response.valid {
        let resp_contract_id = accept_response
            .contract_id
            .parse::<ContractId>()
            .map_err(|err| err.to_string())?;
        let accept_info = AcceptInfo::new(resp_contract_id, rgb_transfer.clone(), None);
        rgb_account.accept_transfer(accept_info);
        debug!(format!(
            "Successfully accepted incoming transfer: {}",
            rgb_transfer.consignment_id()
        ));
    } else {
        // Optionally, update status to "Invalid" or handle as needed
        debug!(format!(
            "Incoming transfer was deemed invalid during acceptance: {}",
            rgb_transfer.consignment_id()
        ));
    }
    Ok(())
}

/// Helper: Processes swap supplements for all open offers and bids.
async fn process_swap_supplements(
    rgb_account: &mut RgbAccount,
    rgb_stock: &mut RGBStock,
    rgb_stock_provider: &mut RGBStockCrossStore,
    _rgb_account_provider: &mut AccountCrossStore,
    rgb_resolver: &mut BitmaskResolver,
) -> Result<(), String> {
    // Combine offers and bids to iterate once
    let public_offers = list_fill_public_offers()
        .await
        .map_err(RgbAccountError::Server)?;

    let offers: Vec<_> = rgb_account
        .offers_by(FilterIncludeAll)
        .iter()
        // Include OPEN, PENDING, and FINISHED offers to handle race condition where
        // offer fills before seller's verify_transfers runs. Skip only CANCELLED offers.
        .filter(|x| !matches!(x.status(), Status::Cancel))
        .cloned()
        .collect();

    for offer in offers {
        if let Some(position) = public_offers.iter().position(|x| x.id == *offer.id()) {
            let public_offer = public_offers[position].clone();

            if let Some(swap_suppl) = public_offer.swap_suppl {
                let SwapSupplement {
                    transfers,
                    tapret,
                    psbt,
                    ..
                } = swap_suppl.clone();

                for (_, transfer) in transfers {
                    let rgb_transfer = Transfer::from_str(&transfer).expect("invalid transfer");
                    prefetch_transfer(&rgb_transfer, rgb_resolver).await?;

                    let req = AcceptRequest {
                        consignment: transfer,
                        force: false,
                    };
                    let accept_resp =
                        accept_rgb_transfer(req, rgb_stock, rgb_account, &*rgb_resolver)
                            .map_err(|err| RgbAccountError::Swap(err.to_string()));

                    extract_transfer(&rgb_transfer)
                        .map_err(|err| RgbAccountError::WrongTransfer(err.to_string()))?;

                    if let Ok(accept_resp) = accept_resp {
                        if accept_resp.valid {
                            let swap_psbt = Psbt::from_base64(&psbt).expect("invalid PSBT");
                            // Diagnostics: enumerate outputs and derivations/mine flags for host resolution
                            {
                                let txid_dbg = swap_psbt.txid();
                                let mut outs_dbg: Vec<String> = vec![];
                                for (i, o) in swap_psbt.outputs().enumerate() {
                                    let spk_hex = o
                                        .script
                                        .as_slice()
                                        .iter()
                                        .map(|b| format!("{:02x}", b))
                                        .collect::<String>();
                                    let der_dbg = if let Some(der) = o.terminal_derivation() {
                                        // Avoid relying on into_u32(); index implements Debug
                                        format!("{:?}/{:?}", der.keychain, der.index)
                                    } else {
                                        "no-derivation".to_string()
                                    };
                                    let host_outpoint =
                                        Outpoint::new(txid_dbg, Vout::from_u32(i as u32));
                                    let is_mine_outpoint =
                                        rgb_account.wallet.outpoint_by(host_outpoint).is_ok();
                                    outs_dbg.push(format!(
                                        "#{} spk={} der={} mine={}",
                                        i, spk_hex, der_dbg, is_mine_outpoint
                                    ));
                                }
                                debug!(format!("swap.psbt_outputs [{}]", outs_dbg.join("; ")));
                            }
                            // Determine asset family from offer and preferred keychain
                            let asset_type = offer.ty();
                            let mut host_keychain = asset_type.to_keychain();
                            let mut host_index_opt: Option<NormalIndex> = None;

                            // A) If offer included a strong host binding (scriptPubKey), locate it and prefer that vout
                            if let Some(offer_suppl) = offer.suppl()
                                && let Some(ref spk_hex) = offer_suppl.seller_host_spk
                            {
                                if let Ok(spk_bytes) = Vec::<u8>::from_hex(spk_hex) {
                                    if let Some((vout_index, _)) =
                                        swap_psbt.to_unsigned_tx().outputs.iter().enumerate().find(
                                            |(_, o)| {
                                                o.script_pubkey.as_slice() == spk_bytes.as_slice()
                                            },
                                        )
                                    {
                                        // Ownership check: this host outpoint must be mine
                                        // If derivation metadata is present on this vout, adopt it
                                        if let Some(der) = swap_psbt
                                            .output(vout_index)
                                            .and_then(|o| o.terminal_derivation())
                                        {
                                            host_keychain = der.keychain;
                                            host_index_opt = Some(der.index);
                                        }
                                    } else {
                                        // Binding missing in merged PSBT: do not abort; fall back to persisted host terminal or derivation scan
                                        debug!(
                                            "swap.host_binding.missing: seller_host_spk not found in merged PSBT; trying terminal/derivation fallback"
                                        );
                                        // Intentionally proceed without setting host_index_opt here
                                    }
                                } else {
                                    return Err("Host binding decode error".to_string());
                                }
                            }

                            // Prefer persisted host terminal from offer supplement
                            if let Some(offer_suppl) = offer.suppl()
                                && let Some(host) = offer_suppl.seller_host_terminal.clone()
                            {
                                host_keychain = host.keychain;
                                host_index_opt = Some(host.index);
                            }

                            // Fallback: scan PSBT outputs for a seller-owned derivation matching asset family
                            let normal_index: NormalIndex = if let Some(idx) = host_index_opt {
                                idx
                            } else {
                                let alt_keychain = asset_type.to_alt_keychain();
                                let mut found: Option<NormalIndex> = None;
                                for o in swap_psbt.outputs() {
                                    if let Some(der) = o.terminal_derivation() {
                                        if der.keychain == host_keychain {
                                            found = Some(der.index);
                                            break;
                                        } else if found.is_none() && der.keychain == alt_keychain {
                                            // Prefer alt family only if primary not found
                                            host_keychain = alt_keychain;
                                            found = Some(der.index);
                                        }
                                    }
                                }
                                found.ok_or(RgbAccountError::TerminalDerivationError)?
                            };

                            debug!(format!(
                                "swap.host_terminal keychain={:?} index={:?}",
                                host_keychain, normal_index
                            ));

                            let txid = swap_psbt.txid();
                            let host_term = Terminal::new(host_keychain, normal_index);

                            // FIX 4: Add tapret to descriptor EARLY to prevent cache corruption
                            // Parse and register tapret immediately after host terminal is determined
                            // This ensures descriptor has the tweak BEFORE transaction could be indexed
                            {
                                let tap_commit =
                                    TapretCommitment::from_str(&tapret).expect("invalid tapret");
                                let host_tap_terminal =
                                    bpwallet::Terminal::new(host_keychain, normal_index);
                                let network = match rgb_account.chain_net() {
                                    ChainNet::BitcoinMainnet => Network::Mainnet,
                                    ChainNet::BitcoinRegtest => Network::Regtest,
                                    ChainNet::BitcoinSignet => Network::Signet,
                                    ChainNet::BitcoinTestnet3 => Network::Testnet3,
                                    ChainNet::BitcoinTestnet4 => Network::Testnet4,
                                    _ => {
                                        return Err(
                                            "Invalid network for address derivation".to_string()
                                        );
                                    }
                                };

                                // Prime derivation indices to ensure wallet knows about this terminal
                                // Check if we need to expand the keychain's derivation index
                                let last_known =
                                    rgb_account.wallet.last_derivation_index(host_keychain);
                                if normal_index >= last_known {
                                    let mut derivation_iter =
                                        rgb_account.wallet.derive_address_range(
                                            AddressNetwork::from(network),
                                            host_keychain,
                                            last_known,
                                            normal_index,
                                        );
                                    while derivation_iter.next().is_some() {}
                                    debug!(format!(
                                        "swap.prime_derivation.early keychain={:?} new_last_index={:?}",
                                        host_keychain,
                                        rgb_account.wallet.last_derivation_index(host_keychain)
                                    ));
                                }

                                // Add tapret to descriptor for host terminal
                                rgb_account.add_tapret(tap_commit.clone(), host_tap_terminal);

                                // Also register the same tapret for other RGB families to preserve extras
                                // This ensures reassigned extras in different families (e.g., RGB20) recognize the same host outpoint as mine.
                                for fam_keychain in [
                                    AssetType::RGB20.to_keychain(),
                                    AssetType::RGB21.to_keychain(),
                                ] {
                                    if fam_keychain != host_keychain {
                                        // Prime derivation for the other family on the same index before saving tapret metadata
                                        let last_known_alt =
                                            rgb_account.wallet.last_derivation_index(fam_keychain);
                                        if normal_index >= last_known_alt {
                                            let mut derivation_iter_alt =
                                                rgb_account.wallet.derive_address_range(
                                                    AddressNetwork::from(network),
                                                    fam_keychain,
                                                    last_known_alt,
                                                    normal_index,
                                                );
                                            while derivation_iter_alt.next().is_some() {}
                                        }
                                        rgb_account.add_tapret(
                                            tap_commit.clone(),
                                            bpwallet::Terminal::new(fam_keychain, normal_index),
                                        );
                                    }
                                }

                                debug!(
                                    "swap.tapret_added_early: descriptor has tweak before transaction indexing"
                                );
                            }

                            // Detect if we're the seller or buyer
                            // Seller created the offer, buyer created a bid for this offer
                            // Only seller should register pending_hosts for their RGB change output
                            let we_are_seller = rgb_account
                                .bids_by(FilterIncludeAll)
                                .iter()
                                .all(|b| b.offer_id() != offer.id());

                            // Add pending_host for swap output using seller_host_spk binding
                            // This works even when PSBT outputs lack terminal_derivation metadata
                            // Only seller registers their RGB change output - buyer must not register seller's output
                            if we_are_seller
                                && let Some(offer_suppl) = offer.suppl()
                                && let Some(ref spk_hex) = offer_suppl.seller_host_spk
                                && let Ok(spk_bytes) = Vec::<u8>::from_hex(spk_hex)
                                && let Some((vout_index, _)) =
                                    swap_psbt.to_unsigned_tx().outputs.iter().enumerate().find(
                                        |(_, o)| o.script_pubkey.as_slice() == spk_bytes.as_slice(),
                                    )
                            {
                                let canonical_outpoint = format!("{}:{}", txid, vout_index);

                                if rgb_account
                                    .is_pending_host_outpoint(&canonical_outpoint)
                                    .is_none()
                                {
                                    rgb_account
                                        .add_pending_host(host_term, canonical_outpoint.clone());
                                    debug!(format!(
                                        "swap.pending_host.via_spk.seller terminal={:?} outpoint={}",
                                        host_term, canonical_outpoint
                                    ));
                                } else {
                                    debug!(format!(
                                        "swap.pending_host.via_spk.already_tracked outpoint={}",
                                        canonical_outpoint
                                    ));
                                }
                            } else if !we_are_seller {
                                debug!(
                                    "swap.pending_host.skip_buyer: buyer does not register seller's output via spk"
                                );
                            }

                            // Import extras contracts BEFORE fascia generation to ensure tapret commitment includes all contract information
                            // This prevents "witness misses contract information in tapret anchor" errors during bundle consumption
                            if let Some(offer_suppl) = offer.suppl()
                                && let Some(serialized) = offer_suppl.clone().extra
                            {
                                let serialized = Vec::<u8>::from_ascii_armored_str(&serialized)
                                    .expect("decode ascii-armored RGB extra");
                                let confined = Confined::from_slice_checked(serialized.as_slice());

                                if let Ok(rgb_shards) =
                                    RgbExtraShards::from_strict_serialized::<U16>(confined)
                                {
                                    let contract_ids: Vec<String> = rgb_shards
                                        .extras
                                        .iter()
                                        .map(|s| s.contract_id.to_string())
                                        .collect();
                                    debug!(format!(
                                        "verify.swap.extras_import.start count={} contracts=[{}]",
                                        rgb_shards.extras.len(),
                                        contract_ids.join(", ")
                                    ));

                                    for extra_shard in &rgb_shards.extras {
                                        let contract_id = extra_shard.contract_id;
                                        let contract = extra_shard.contract.clone();

                                        // Validate and import contract into rgb_stock so rgb_commit() includes it in tapret
                                        let type_system =
                                            AssetSchema::from(contract.schema_id()).types();
                                        let chain_net = rgb_account.chain_net();
                                        match contract.validate(
                                            rgb_resolver,
                                            &crate::rgb::compat::validation_config(
                                                chain_net,
                                                type_system,
                                            ),
                                        ) {
                                            Ok(valid_contract) => {
                                                match rgb_stock.import_contract(
                                                    valid_contract,
                                                    rgb_resolver.clone(),
                                                ) {
                                                    Ok(_) => {
                                                        debug!(format!(
                                                            "verify.swap.extras_import.success contract_id={}",
                                                            contract_id
                                                        ));
                                                    }
                                                    Err(e) => {
                                                        // Contract may already exist - log but don't fail
                                                        debug!(format!(
                                                            "verify.swap.extras_import.already_exists contract_id={} reason={:?}",
                                                            contract_id, e
                                                        ));
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                // Validation failed - log and skip
                                                debug!(format!(
                                                    "verify.swap.extras_import.validation_failed contract_id={} reason={:?}",
                                                    contract_id, e
                                                ));
                                            }
                                        }
                                    }
                                    debug!(format!(
                                        "verify.swap.extras_import.complete imported_count={}",
                                        rgb_shards.extras.len()
                                    ));
                                }
                            }

                            // Extract fascia from SwapSupplement or try to get it from PSBT
                            let fascia_for_extras = if let Some(ref fascia_str) = swap_suppl.fascia
                            {
                                // Deserialize fascia from SwapSupplement
                                debug!(
                                    "verify.swap.fascia.deserialize attempt from swap_suppl"
                                        .to_string()
                                );
                                let fascia_opt = Vec::<u8>::from_ascii_armored_str(fascia_str)
                                    .ok()
                                    .and_then(|bytes| {
                                        let confined =
                                            Confined::from_slice_checked(bytes.as_slice());
                                        rgbstd::containers::Fascia::from_strict_serialized::<U16>(
                                            confined,
                                        )
                                        .ok()
                                    });
                                if fascia_opt.is_some() {
                                    debug!("verify.swap.fascia.deserialize SUCCESS".to_string());
                                } else {
                                    debug!("verify.swap.fascia.deserialize FAILED".to_string());
                                }
                                fascia_opt
                            } else {
                                // Fallback: try to get fascia from PSBT (may fail if finalized)
                                debug!("verify.swap.fascia.fallback no fascia in swap_suppl, trying rgb_commit".to_string());
                                let mut swap_psbt_for_fascia = swap_psbt.clone();
                                swap_psbt_for_fascia.rgb_commit().ok()
                            };

                            // Ingest anchor (fascia) for the swap witness AFTER importing extras contracts
                            // so the tapret commitment includes all contract information
                            {
                                // Minimal resolver declaring the swap witness as Tentative (same pattern as builder side)
                                use crate::rgb::compat::TentativeWitness;

                                // Use fascia from SwapSupplement (authoritative fascia created by swap builder)
                                // rgb_commit() is not needed here - we're verifying an existing commitment, not creating one
                                // The fascia contains all bundles: main swap contract + extras contracts
                                let fascia_to_consume = fascia_for_extras.clone();

                                match fascia_to_consume {
                                    Some(fascia) => {
                                        // Consume the fascia anchor so bundles attached to this witness integrate into validated state
                                        match rgb_stock
                                            .consume_fascia(fascia, TentativeWitness::from_bp(txid))
                                        {
                                            Ok(_) => {
                                                debug!(format!(
                                                    "verify.swap.consume_fascia witness_id={} offer_status={:?}",
                                                    txid,
                                                    offer.status()
                                                ));
                                            }
                                            Err(e) => {
                                                debug!(format!(
                                                    "verify.swap.consume_fascia.error witness_id={} offer_status={:?} err={:?}",
                                                    txid,
                                                    offer.status(),
                                                    e
                                                ));
                                            }
                                        }
                                    }
                                    None => {
                                        debug!(format!(
                                            "verify.swap.consume_fascia.skip witness_id={} offer_status={:?} no_fascia_available",
                                            txid,
                                            offer.status()
                                        ));
                                    }
                                }
                            }
                            // Diagnose host outpoint ownership across wallet (post host_terminal selection)
                            // Only seller registers their RGB change output - buyer must not register seller's output
                            if we_are_seller {
                                // Find the specific vout that matches the selected host terminal derivation
                                let host_vout_opt =
                                    swap_psbt.outputs().enumerate().find_map(|(i, o)| {
                                        if let Some(der) = o.terminal_derivation() {
                                            if der.keychain == host_keychain
                                                && der.index == normal_index
                                            {
                                                Some(Vout::from_u32(i as u32))
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    });

                                if let Some(host_vout) = host_vout_opt {
                                    let host_outpoint = Outpoint::new(txid, host_vout);
                                    // Outpoint-based ownership may be false until the indexer sees the new UTXO.
                                    // Since host_vout was found via terminal_derivation() matching our host terminal,
                                    // treat derivation match as authoritative "mine" signal for immediate processing.
                                    let is_mine_outpoint =
                                        rgb_account.wallet.outpoint_by(host_outpoint).is_ok();
                                    let is_mine_derivation = true; // host_vout_opt was selected by matching (host_keychain, normal_index)
                                    debug!(format!(
                                        "swap.host_outpoint.seller outpoint={} mine_outpoint={} mine_derivation={}",
                                        host_outpoint, is_mine_outpoint, is_mine_derivation
                                    ));
                                    // Record pending host so exporters can treat derivation+tapret as mine pre-index
                                    if !is_mine_outpoint {
                                        let host_term = Terminal::new(host_keychain, normal_index);
                                        // Persist a canonical outpoint string "txid:vout" to match exporter formatting (bpwallet::Outpoint::to_string)
                                        let canonical_outpoint =
                                            format!("{}:{}", txid, host_vout.into_u32());

                                        // Check if already tracked to avoid redundant logging
                                        if rgb_account
                                            .is_pending_host_outpoint(&canonical_outpoint)
                                            .is_none()
                                        {
                                            rgb_account.add_pending_host(
                                                host_term,
                                                canonical_outpoint.clone(),
                                            );
                                            debug!(format!(
                                                "swap.pending_host.new.seller terminal={:?} outpoint={}",
                                                host_term, canonical_outpoint
                                            ));
                                        } else {
                                            debug!(format!(
                                                "swap.pending_host.already_tracked outpoint={}",
                                                canonical_outpoint
                                            ));
                                        }
                                    }
                                    // Proceed when either outpoint is known or derivation matched our wallet.
                                    if !is_mine_outpoint && !is_mine_derivation {
                                        return Err("Selected host outpoint is not owned by seller; refusing extras consumption".to_string());
                                    }
                                } else {
                                    debug!(
                                        "swap.host_outpoint.seller=(no matching vout for selected host terminal)"
                                    );
                                }
                            } else {
                                debug!(
                                    "swap.pending_host.skip_buyer_derivation: buyer does not register seller's terminal"
                                );
                            }
                            if let Some(offer_suppl) = offer.suppl() {
                                let extra_shards: Vec<ContractId> = if let Some(serialized) =
                                    offer_suppl.clone().extra
                                {
                                    let serialized = Vec::<u8>::from_ascii_armored_str(&serialized)
                                        .expect("decode ascii-armored RGB extra");
                                    let confined =
                                        Confined::from_slice_checked(serialized.as_slice());

                                    let rgb_shards =
                                        RgbExtraShards::from_strict_serialized::<U16>(confined)
                                            .expect("decode ascii-armored RGB extra");
                                    rgb_shards.extras.iter().map(|x| x.contract_id).collect()
                                } else {
                                    vec![]
                                };

                                let extra_bundles = swap_psbt.rgb_bundles().expect("invalid PSBT");
                                debug!(format!(
                                    "swap.extra_shards.seller={:?}",
                                    extra_shards
                                        .iter()
                                        .map(|x| x.to_string())
                                        .collect::<Vec<_>>()
                                ));
                                let mut seen_bundle_contracts: Vec<String> = vec![];
                                for (contract_id, bundle) in extra_bundles {
                                    seen_bundle_contracts.push(contract_id.to_string());
                                    if !extra_shards.contains(&contract_id) {
                                        debug!(format!(
                                            "swap.extra_bundle.skip contract_id={}",
                                            contract_id
                                        ));
                                        continue;
                                    }

                                    debug!(format!(
                                        "swap.extra_bundle.consume contract_id={}",
                                        contract_id
                                    ));

                                    // Create AnchorShard for this extras contract using the swap witness
                                    // The fascia contains the SealWitness with the tapret commitment proof
                                    if let Some(ref fascia) = fascia_for_extras {
                                        let anchor_shard = AnchorShard {
                                            witness_seal: fascia.seal_witness().clone(),
                                            bundle: bundle.clone(),
                                        };

                                        debug!(format!(
                                            "swap.extra_anchor.consume contract_id={} witness_id={}",
                                            contract_id, txid
                                        ));

                                        // Consume the anchor (which includes bundle + witness proof)
                                        rgb_stock
                                            .consume_anchor_raw(
                                                contract_id,
                                                anchor_shard,
                                                txid,
                                                rgb_stock_provider,
                                            )
                                            .map_err(|e| {
                                                debug!(format!(
                                                    "swap.extra_anchor.error contract_id={} err={:?}",
                                                    contract_id, e
                                                ));
                                                e
                                            })
                                            .expect("Failed to consume extras anchor");

                                        // Reload rgb_stock from provider to get the updated state with swap anchors
                                        *rgb_stock = RGBStock::get(rgb_stock_provider.clone())
                                            .expect(
                                                "Failed to reload stock after anchor consumption",
                                            );

                                        debug!(format!(
                                            "swap.extra_anchor.success contract_id={}",
                                            contract_id
                                        ));
                                    } else {
                                        // Fallback to bundle-only consumption if fascia unavailable
                                        debug!(format!(
                                            "swap.extra_bundle.fallback contract_id={} reason=no_fascia",
                                            contract_id
                                        ));
                                        rgb_stock
                                            .consume_bundle_raw(
                                                contract_id,
                                                bundle,
                                                txid,
                                                rgb_stock_provider,
                                            )
                                            .expect("Failed to consume extras bundle");
                                        *rgb_stock = RGBStock::get(rgb_stock_provider.clone())
                                            .expect(
                                                "Failed to reload stock after bundle consumption",
                                            );
                                    }
                                }
                                debug!(format!(
                                    "swap.extra_bundles.contract_ids={:?}",
                                    seen_bundle_contracts
                                ));
                            }

                            // Persist durable lookahead hint for this host terminal
                            rgb_account.add_lookahead_hint(host_keychain, normal_index);
                            debug!(format!(
                                "swap.lookahead_hint.persisted keychain={:?} index={:?}",
                                host_keychain, normal_index
                            ));

                            let mut offer_updated = offer.clone();
                            offer_updated.add_swap_suppl(swap_suppl.clone());
                            offer_updated.complete();

                            rgb_account.update_offer(offer_updated.clone());
                            debug!(format!("offer filled {}", offer_updated.id().to_string()));
                        }
                    } else {
                        println!("offer not complete {:?}", offer.id());
                        println!("offer validation: {:#?}", accept_resp.err());
                    }
                }
            }
        }
    }

    let bids: Vec<_> = rgb_account
        .bids_by(FilterIncludeAll)
        .iter()
        .filter(|x| x.is_open())
        .cloned()
        .collect();

    for bid in bids {
        if let Some(swap_suppl) = bid.swap() {
            let SwapSupplement { transfers, .. } = swap_suppl.clone();

            for (_, transfer) in transfers {
                let rgb_transfer = Transfer::from_str(&transfer).expect("invalid transfer");
                prefetch_transfer(&rgb_transfer, rgb_resolver).await?;

                let req = AcceptRequest {
                    consignment: transfer,
                    force: false,
                };

                let accept_resp = accept_rgb_transfer(req, rgb_stock, rgb_account, &*rgb_resolver)
                    .map_err(|err| RgbAccountError::Swap(err.to_string()));

                if let Ok(accept_resp) = accept_resp {
                    if accept_resp.valid {
                        let mut bid_updated = bid.clone();
                        bid_updated.complete();

                        rgb_account.update_bid(bid_updated.clone());
                        debug!(format!("bid filled {}", bid_updated.id().to_string()));
                    }
                } else if let Err(accept_err) = accept_resp {
                    debug!(format!("bid not complete {}", bid.id().to_string()));
                    debug!(format!("bid bad validations {:#?}", accept_err));
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum InvoiceError {
    /// some request data is invalid. {0:?}
    Validation(BTreeMap<String, String>),
    /// I/O or connectivity error. {0}
    IO(CrossStoreError),
    /// explorer sync error. {0}
    Explorer(String),
    /// account sync error. {0}
    Wallet(String),
    /// stock sync error. {0}
    Stock(String),
    /// store retrieve error. {0}
    Store(String),
    /// utxo not found.
    NoUtxo,
    /// address not found.
    NoAddress,
    /// wrong rgb selection.
    WrongSelection,
    /// invalid rgb invoice. {0}
    InvalidInvoice(String),
    /// prefetch error. {0}
    PrefetchError(#[from] prefetch::PrefetchError),
    /// outpoint parse error. {0}
    OutpointParseError(#[from] bpstd::OutpointParseError),
    /// address parse error. {0}
    AddressParseError(#[from] bpstd::AddressParseError),
    /// rgb asset parse error. {0}
    AssetParseError(#[from] AssetParseError),
}

pub async fn create_invoice(
    sk: &str,
    request: RgbInvoiceRequest,
    options: RgbInvoiceOptions,
) -> Result<RgbInvoiceResponse, InvoiceError> {
    if let Err(err) = request.validate() {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(InvoiceError::Validation(errors));
    }

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| InvoiceError::Explorer(err.to_string()))?;

    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| InvoiceError::Explorer(err.to_string()))?;

    let (mut rgb_account_provider, mut rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| InvoiceError::Store(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| InvoiceError::Store(err.to_string()))?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| InvoiceError::Store(err.to_string()))?;

    let rgb_info = create_rgb_invoice(request, options, &mut rgb_stock, &mut rgb_account)?;
    rgb_account.add_invoice(rgb_info.clone());
    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| InvoiceError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| InvoiceError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| InvoiceError::Store(err.to_string()))?;

    rgb_stock_provider
        .persist()
        .await
        .map_err(|err| InvoiceError::Store(err.to_string()))?;

    Ok(RgbInvoiceResponse {
        invoice: rgb_info.invoice.to_string(),
    })
}

pub async fn remove_invoice(
    sk: &str,
    request: String,
    _: RetrieveContractOptions,
) -> Result<(), InvoiceError> {
    bp_network_from_str(&get_network().await)
        .map_err(|err| InvoiceError::Explorer(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(InvoiceError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(InvoiceError::IO)?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| InvoiceError::Wallet(err.to_string()))?;

    let rgb_invoice = RgbInvoice::from_str(&request)
        .map_err(|err| InvoiceError::InvalidInvoice(err.to_string()))?;

    rgb_account.remove_invoice(rgb_invoice);
    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| InvoiceError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| InvoiceError::Store(err.to_string()))?;

    Ok(())
}

pub async fn list_invoices(
    sk: &str,
    _: RetrieveContractOptions,
) -> Result<Vec<RgbInvoiceStatusResponse>, InvoiceError> {
    bp_network_from_str(&get_network().await)
        .map_err(|err| InvoiceError::Explorer(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(InvoiceError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(InvoiceError::IO)?;

    let rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| InvoiceError::Wallet(err.to_string()))?;

    let mut my_invoices = vec![];
    for RgbInvoiceItem {
        invoice: rgb_invoice,
        payed: status,
        ..
    } in rgb_account.invoices_by(FilterIncludeAll)
    {
        // Get contract info if available, otherwise use placeholders
        #[allow(clippy::collapsible_if)]
        // inner else is unknown contract; outer else is any-asset invoice
        let (asset_name, asset_ticker, precision) =
            if let Some(contract_id) = rgb_invoice.to_owned().contract {
                if let Some(contract_info) = rgb_account.contract_by(contract_id) {
                    (
                        contract_info.name().to_string(),
                        contract_info.ticker().to_string(),
                        *contract_info.precision(),
                    )
                } else {
                    // Contract ID exists but not in account (unknown asset)
                    ("Unknown".to_string(), "???".to_string(), 0)
                }
            } else {
                // Blinded invoice without specific contract (can receive any asset)
                ("Any Asset".to_string(), "ANY".to_string(), 0)
            };

        my_invoices.push(RgbInvoiceStatusResponse::new(
            &rgb_invoice,
            &status,
            &asset_name,
            &asset_ticker,
            &precision,
        ));
    }

    Ok(my_invoices)
}

pub async fn decode_invoice(invoice: String) -> Result<RgbInvoiceDecoded, InvoiceError> {
    let rgb_invoice = RgbInvoice::from_str(&invoice).map_err(|_| {
        InvoiceError::Validation(
            bmap! { "rgb_invoice".to_string() => "invoice parse error".to_string() },
        )
    })?;

    let contract_id = rgb_invoice
        .contract
        .map(|x| x.to_string())
        .unwrap_or_default();

    let amount = match rgb_invoice.assignment_state {
        Some(InvoiceState::Amount(amount)) => {
            let amount: u64 = amount.into();
            RgbInvoiceValue::Value(format!("{amount}"))
        }
        Some(InvoiceState::Data(NonFungible::FractionedToken(allocation))) => {
            RgbInvoiceValue::UDA(allocation.into())
        }
        _ => {
            // For blinded open invoices there is no fixed assignment.
            // Map to zero for backward-compatible decoding.
            RgbInvoiceValue::Value("0".to_string())
        }
    };

    Ok(RgbInvoiceDecoded {
        contract_id,
        amount,
    })
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum TransferError {
    /// some request data is invalid. {0:?}
    Validation(BTreeMap<String, String>),
    /// I/O or connectivity error. {0}
    IO(CrossStoreError),
    /// store retrieve error. {0}
    Store(String),
    /// explorer sync error. {0}
    Explorer(String),
    /// account sync error. {0}
    Wallet(String),
    /// transfers sync error. {0}
    Transfers(String),
    /// stock sync error. {0}
    Stock(String),
    /// proxy sync error. {0}
    Proxy(ProxyError),
    /// parse invoice fail. {0}
    Invoice(String),
    /// payment fail. {0}
    Pay(String),
    /// swap fail. {0}
    Swap(String),
    /// psbt broadcast fail. {0}
    Publish(String),
    /// fee estimate error. {0}
    FeeEstimate(String),
    /// rgb stock error. {0}
    StockError(String),
    /// rgb stock error. {0}
    PersistenceError(String),
    /// rgb validation error, no fund vault. {0}
    NoFundVault(String),
    /// rgb watcher error. {0}
    RgbWatcherError(#[from] WatcherError),
    /// allocation outpoint not present
    NoOutpointForAllocation,
    /// baid64 parse error. {0}
    ContractParseError(String),
    /// prefetch error. {0}
    PrefetchError(#[from] prefetch::PrefetchError),
    /// rgb validation error. {0}
    RgbValidationError(#[from] RgbValidation),
}

pub async fn can_transfer_contract(
    sk: &str,
    request: RgbTransferRequest,
    _: RgbTransferOptions,
) -> Result<RgbValidationResponse, TransferError> {
    if let Err(err) = request.validate() {
        let _errors: Vec<_> = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
    }

    let _rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| TransferError::Explorer(err.to_string()))?;

    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| TransferError::Explorer(err.to_string()))?;

    let (rgb_account_provider, rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| TransferError::Store(err.to_string()))?;

    let rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| TransferError::Store(err.to_string()))?;

    let rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| TransferError::Store(err.to_string()))?;

    let mut result = RgbValidationResponse::default();

    let RgbTransferRequest {
        contract_id,
        iface,
        invoice,
        sender_amount,
        ..
    } = request;

    let next_utxo = watcher_next_utxo(sk, &iface.to_string()).await?;
    if next_utxo.utxo.is_none() {
        result.failure(RgbValidation::NoFundVault.to_string());
        return Err(TransferError::RgbValidationError(
            RgbValidation::NoFundVault,
        ));
    }
    let next_utxo = next_utxo.utxo.unwrap_or_default();
    let output_seal = if let Ok(outpoint) = Outpoint::from_str(&next_utxo) {
        OutputSeal::new(crate::rgb::compat::outpoint_bp_to_bitcoin(outpoint))
    } else {
        result.failure(RgbValidation::InvalidUtxo(next_utxo.clone()).to_string());
        return Err(TransferError::RgbValidationError(
            RgbValidation::InvalidUtxo(next_utxo),
        ));
    };

    let filter_open_invoices = FilterOutpoint::new(output_seal.to_outpoint(), Status::Open);
    let open_seals = rgb_account.invoices_by(filter_open_invoices);
    if !open_seals.is_empty() {
        let seals = open_seals.iter().map(|x| x.seal.clone()).collect();
        result.failure(RgbValidation::OpenSeals(seals).to_string());
    }

    if !rgb_account
        .transfers_by(output_seal.to_outpoint())
        .is_empty()
    {
        result.failure(RgbValidation::UtxoInUse(next_utxo.clone().to_string()).to_string());
        return Err(TransferError::RgbValidationError(RgbValidation::UtxoInUse(
            next_utxo.clone().to_string(),
        )));
    }

    // Consider only transfers that are still marked as OPEN
    let pending = rgb_account.transfers_by(FilterStatus::new(Status::Open));

    debug!(format!("pending transfers {pending:?} {iface}"));
    if !pending.is_empty() {
        let transfers: Vec<_> = rgb_account
            .transfers_by(FilterStatus::new(Status::Open))
            .iter()
            .filter(|x| x.transfer_id().is_some())
            .map(|x| x.transfer_id().unwrap().to_string())
            .collect();
        result.warning(RgbValidation::OpenTransfers(transfers).to_string());
    }

    let contract_id = ContractId::from_str(&contract_id)
        .map_err(|e| TransferError::ContractParseError(e.to_string()))?;

    let is_contract = rgb_stock
        .contracts()
        .map_err(|_e| TransferError::StockError(contract_id.to_string()))?
        .any(|x| x.id == contract_id);

    if !is_contract {
        result.failure(RgbValidation::NoContract(contract_id.to_string()).to_string());
        return Err(TransferError::RgbValidationError(
            RgbValidation::NoContract(contract_id.to_string()),
        ));
    }

    let rgb_contract = rgb_stock.contract_data(contract_id).map_err(|_e| {
        TransferError::RgbValidationError(RgbValidation::NoContract(contract_id.to_string()))
    })?;

    let contract_allocation = rgb_contract
        .allocations(&FilterIncludeAll)
        .collect::<Vec<_>>();

    let rgb_wallet = rgb_account.wallet;
    // Choose keychains based on iface (RGB20 or RGB21)
    let asset_type = AssetType::from_str(&iface).unwrap_or(AssetType::RGB20);
    let rgb_keychains = [asset_type.to_keychain(), asset_type.to_alt_keychain()];

    let available_utxos: Vec<_> = rgb_wallet
        .utxos()
        .filter(|u| {
            matches!(u.status, TxStatus::Mined(_)) && rgb_keychains.contains(&u.terminal.keychain)
        })
        .map(|u| u.outpoint)
        .collect();

    let rgb_invoice = RgbInvoice::from_str(&invoice).map_err(|_e| {
        TransferError::RgbValidationError(RgbValidation::InvalidInvoice(invoice.to_string()))
    })?;

    // Determine the requested state:
    // - Use invoice assignment_state if present and non-zero (fungible)
    // - Treat Amount(0) as "open" and prefer sender_amount if provided
    // - If missing/zero and no sender_amount, fail validation
    let requested_state = rgb_invoice.assignment_state.clone();

    // RGB21 blinded preflight: treat senderAmount.value as integer count, default to 1
    // and avoid routing into the fungible path. Non-blinded RGB21 without a concrete UDA
    // allocation in the invoice is invalid.
    let is_rgb21 = matches!(asset_type, AssetType::RGB21);
    if is_rgb21 {
        let is_blinded = matches!(
            rgb_invoice.beneficiary.into_inner(),
            Beneficiary::BlindedSeal(_)
        );

        match &requested_state {
            // Exact UDA specified: fall through to the RGB21 UDA branch below.
            Some(InvoiceState::Data(NonFungible::FractionedToken(_))) => {}
            _ => {
                if !is_blinded {
                    // Non-blinded RGB21 requires UDA allocation in the invoice.
                    result.failure(RgbValidation::InvalidInvoice(invoice.to_string()).to_string());
                    return Err(TransferError::RgbValidationError(
                        RgbValidation::InvalidInvoice(invoice.to_string()),
                    ));
                }

                // Determine requested count
                let requested_count: u64 = if let Some(sa) = sender_amount {
                    match sa {
                        RgbInvoiceValue::Value(v) => {
                            // Reject decimals and non-integer formats
                            if v.parse::<u64>().ok().filter(|c| *c > 0).is_none() {
                                result.failure(
                                    RgbValidation::InvalidInvoice(
                                        "RGB21 blinded requires integer token count >= 1"
                                            .to_string(),
                                    )
                                    .to_string(),
                                );
                                return Err(TransferError::RgbValidationError(
                                    RgbValidation::InvalidInvoice(
                                        "RGB21 blinded requires integer token count >= 1"
                                            .to_string(),
                                    ),
                                ));
                            }
                            v.parse::<u64>().unwrap()
                        }
                        RgbInvoiceValue::UDA(_) => {
                            // Sender-provided UDA for blinded invoices is not allowed.
                            result.failure(
                                RgbValidation::InvalidInvoice(
                                    "Do not supply UDA for RGB21 blinded; use integer count"
                                        .to_string(),
                                )
                                .to_string(),
                            );
                            return Err(TransferError::RgbValidationError(
                                RgbValidation::InvalidInvoice(
                                    "Do not supply UDA for RGB21 blinded; use integer count"
                                        .to_string(),
                                ),
                            ));
                        }
                    }
                } else {
                    // Default to 1 when absent
                    1
                };

                // Availability check across mined UTXOs: count UDA allocations (fraction > 0)
                let allocation_name = FieldName::from_str("assetOwner").unwrap();
                let uda_allocation = rgb_contract
                    .data(allocation_name, &FilterIncludeAll)
                    .map_err(|_e| TransferError::StockError("contract data".to_string()))?
                    .collect::<Vec<_>>();

                let mut available_count: u64 = 0;
                for a in uda_allocation {
                    if let Some(outpoint) = a.seal.outpoint()
                        && available_utxos
                            .contains(&crate::rgb::compat::outpoint_bitcoin_to_bp(outpoint))
                    {
                        let allocation: Allocation = a.state.into();
                        let fraction: u64 = allocation.fraction().into();
                        if fraction > 0 {
                            available_count += 1;
                        }
                    }
                }

                if available_count < requested_count {
                    result.failure(
                        RgbValidation::NoAllocation(
                            requested_count.to_string(),
                            available_count.to_string(),
                        )
                        .to_string(),
                    );
                    return Err(TransferError::RgbValidationError(
                        RgbValidation::NoAllocation(
                            requested_count.to_string(),
                            available_count.to_string(),
                        ),
                    ));
                }

                // Success for RGB21-blinded preflight (count-based). Do not fall into fungible path.
                return Ok(result);
            }
        }
    }

    // Non-RGB21 (fungible) or RGB21 with concrete UDA allocation proceeds below.

    match requested_state {
        Some(InvoiceState::Amount(fungible_amount)) => {
            // Original fungible validation logic
            let mut total_amount = Amount::ZERO;
            for allocation in contract_allocation {
                let OwnedAllocation { state, seal, .. } = allocation.clone();
                let Some(allocation_outpoint) = seal.outpoint() else {
                    result.warning(TransferError::NoOutpointForAllocation.to_string());
                    return Err(TransferError::NoOutpointForAllocation);
                };

                match state {
                    AllocatedState::Amount(amount) => {
                        if available_utxos.contains(&crate::rgb::compat::outpoint_bitcoin_to_bp(
                            allocation_outpoint,
                        )) {
                            total_amount += amount.into();
                        }
                    }
                    _ => {
                        result
                            .failure(RgbValidation::NoSupport(contract_id.to_string()).to_string());
                        return Err(TransferError::RgbValidationError(RgbValidation::NoSupport(
                            contract_id.to_string(),
                        )));
                    }
                };
            }

            if total_amount < fungible_amount {
                result.failure(
                    RgbValidation::NoAllocation(
                        fungible_amount.to_string(),
                        total_amount.to_string() + "one",
                    )
                    .to_string(),
                );
                return Err(TransferError::RgbValidationError(
                    RgbValidation::NoAllocation(
                        fungible_amount.to_string(),
                        total_amount.to_string() + "two",
                    ),
                ));
            }

            Ok(result)
        }
        Some(InvoiceState::Data(NonFungible::FractionedToken(req_alloc))) => {
            // Exact UDA match: token_index and fraction must be available and anchored to our mined UTXO
            let req_token: u32 = req_alloc.token_index().into();
            let req_fraction: u64 = req_alloc.fraction().into();

            let available_utxos: Vec<_> = rgb_wallet
                .utxos()
                .filter(|u| {
                    matches!(u.status, TxStatus::Mined(_))
                        && rgb_keychains.contains(&u.terminal.keychain)
                })
                .map(|u| u.outpoint)
                .collect();

            let allocation_name = FieldName::from_str("assetOwner").unwrap();
            let uda_allocation = rgb_contract
                .data(allocation_name, &FilterIncludeAll)
                .map_err(|_e| TransferError::StockError("contract data".to_string()))?
                .collect::<Vec<_>>();

            let mut found_match = false;
            let mut available_matches = 0u64;

            for a in uda_allocation {
                /* use fields directly */
                let Some(outpoint) = a.seal.outpoint() else {
                    continue;
                };
                if !available_utxos.contains(&crate::rgb::compat::outpoint_bitcoin_to_bp(outpoint))
                {
                    continue;
                }
                let allocation: Allocation = a.state.into();
                let token_index: u32 = allocation.token_index().into();
                let fraction: u64 = allocation.fraction().into();
                if fraction > 0 {
                    available_matches += 1;
                }
                if token_index == req_token && fraction >= req_fraction {
                    found_match = true;
                    break;
                }
            }

            if !found_match {
                result.failure(
                    RgbValidation::NoAllocation(
                        format!("{}@{}", req_token, req_fraction),
                        available_matches.to_string(),
                    )
                    .to_string(),
                );
                return Err(TransferError::RgbValidationError(
                    RgbValidation::NoAllocation(
                        format!("{}@{}", req_token, req_fraction),
                        available_matches.to_string(),
                    ),
                ));
            }

            Ok(result)
        }
        _ => {
            result.failure(RgbValidation::InvalidInvoice(invoice.to_string()).to_string());
            Err(TransferError::RgbValidationError(
                RgbValidation::InvalidInvoice(invoice.to_string()),
            ))
        }
    }
}

pub async fn transfer_contract(
    sk: &str,
    request: RgbTransferRequest,
    options: RgbTransferOptions,
) -> Result<RgbTransferResponse, TransferError> {
    if let Err(err) = request.validate() {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(TransferError::Validation(errors));
    }

    let _rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| TransferError::Explorer(err.to_string()))?;

    bp_network_from_str(&get_network().await)
        .map_err(|err| TransferError::Explorer(err.to_string()))?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(TransferError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| TransferError::Stock(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| TransferError::Stock(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(TransferError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(TransferError::IO)?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| TransferError::Wallet(err.to_string()))?;

    // Prepare sender amount for proxy publishing; for RGB21 blinded default to 1 when absent,
    // reject UDA form, and require integer count >= 1 when provided.
    let mut sender_amount_for_proxy = request.sender_amount.clone();
    let asset_type_for_proxy = AssetType::from_str(&request.iface).unwrap_or(AssetType::RGB20);
    debug!(format!("IFACE {asset_type_for_proxy}"));
    if matches!(asset_type_for_proxy, AssetType::RGB21)
        && let Ok(tmp_invoice) = RgbInvoice::from_str(&request.invoice)
        && let Beneficiary::BlindedSeal(_) = tmp_invoice.beneficiary.into_inner()
    {
        match &sender_amount_for_proxy {
            Some(crate::structs::RgbInvoiceValue::Value(v)) => {
                if v.parse::<u64>().ok().filter(|c| *c > 0).is_none() {
                    return Err(TransferError::Validation(bmap! {
                        "sender_amount".to_string() => "RGB21 blinded requires integer token count >= 1".to_string()
                    }));
                }
            }
            Some(crate::structs::RgbInvoiceValue::UDA(_)) => {
                return Err(TransferError::Validation(bmap! {
                    "sender_amount".to_string() => "Do not supply UDA for RGB21 blinded; use integer count".to_string()
                }));
            }
            None => {
                // Default to 1 for RGB21 blinded when not provided
                sender_amount_for_proxy =
                    Some(crate::structs::RgbInvoiceValue::Value("1".to_string()));
            }
        }
    }

    let rgb_transfer = create_rgb_transfer(
        request.clone(),
        options.clone(),
        &mut rgb_stock,
        &mut rgb_account,
    )
    .await?;

    let RgbTransferResult { psbt, consig, .. } = rgb_transfer.clone();

    let RgbTransferOptions {
        sign,
        psbt_version,
        private_keys,
        ..
    } = options;

    if sign {
        let broadcast_req = SignPsbtRequest {
            psbt: psbt.to_base64_ver(PsbtVer::V0),
            descriptors: private_keys,
        };

        sign_and_publish_psbt_file_bp(broadcast_req)
            .await
            .map_err(|op| TransferError::Publish(op.to_string()))?;
    };

    if options.proxy {
        let rgb_invoice = RgbInvoice::from_str(&request.invoice)
            .map_err(|err| TransferError::Invoice(err.to_string()))?;

        publish_rgb_transfer(rgb_invoice.clone(), consig.clone(), sender_amount_for_proxy).await?;

        if let Beneficiary::BlindedSeal(seal) = rgb_invoice.beneficiary.into_inner() {
            debug!(format!("consignment sent to proxy: {seal}"));
        };
    }

    let RgbTransferResult {
        consig: rgb_transfer,
        psbt,
    } = rgb_transfer;
    let rgb_invoice = RgbInvoice::from_str(&request.invoice)
        .map_err(|op| TransferError::Invoice(op.to_string()))?;

    if let Some(contract_info) = rgb_account.contract_by(rgb_transfer.contract_id()) {
        let rgb_info = RgbTransferInfo::new(
            rgb_transfer,
            rgb_invoice,
            *contract_info.precision(),
            Some(psbt.clone()),
        );

        rgb_account.add_transfer(rgb_info);
    }

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| TransferError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| TransferError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| TransferError::Store(err.to_string()))?;

    rgb_stock_provider
        .persist()
        .await
        .map_err(|err| TransferError::Store(err.to_string()))?;

    Ok(RgbTransferResponse {
        consig_id: consig.consignment_id().to_string(),
        consig: consig.to_string(),
        psbt: psbt.to_base64_ver(psbt_version),
        txid: psbt.txid().to_string(),
        commit: None,
    })
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum AcceptError {
    /// some request data is invalid. {0:?}
    Validation(BTreeMap<String, String>),
    /// I/O or connectivity error. {0}
    IO(CrossStoreError),
    /// explorer sync error. {0}
    Explorer(String),
    /// account sync error. {0}
    Wallet(String),
    /// stock sync error. {0}
    Stock(String),
    /// store sync error. {0}
    Store(String),
    /// The contract '{0}' does not exist or does not be imported
    NoContract(String),
    /// The transfer '{0}' does not have an invoice related
    NoCounterParty(String),
    /// consignment parse error. {0}
    WrongConsig(String),
    /// invalid consignment. {0}
    WrongTransfer(String),
    /// accept fail. {0}
    Accept(String),
    /// prefetch error. {0}
    PrefetchError(#[from] prefetch::PrefetchError),
    /// resolver error. {0}
    ResolverError(#[from] ResolverError),
}

pub async fn accept_transfer(
    sk: &str,
    request: AcceptRequest,
    _: RgbAcceptOptions,
) -> Result<AcceptResponse, AcceptError> {
    if let Err(err) = request.validate() {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(AcceptError::Validation(errors));
    }

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| AcceptError::Explorer(err.to_string()))?;

    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| AcceptError::Explorer(err.to_string()))?;

    let (mut rgb_account_provider, mut rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| AcceptError::Explorer(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| AcceptError::Stock(err.to_string()))?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| AcceptError::Wallet(err.to_string()))?;

    let transfer = Transfer::from_str(&request.consignment)
        .map_err(|op| AcceptError::WrongConsig(op.to_string()))?;

    prefetch_transfer(&transfer, &mut rgb_resolver).await?;

    let resp = accept_rgb_transfer(
        request.clone(),
        &mut rgb_stock,
        &mut rgb_account,
        &rgb_resolver,
    )?;
    let AcceptResponse { contract_id, .. } = resp.clone();

    let contract_id = ContractId::from_str(&contract_id).map_err(|_| {
        AcceptError::Validation(
            bmap! { "contract_id".to_string() => "parameter is invalid (parse error)".to_string() },
        )
    })?;

    let rgb_info = AcceptInfo::new(contract_id, transfer, None);
    rgb_account.accept_transfer(rgb_info);
    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| AcceptError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| AcceptError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| AcceptError::Store(err.to_string()))?;

    rgb_stock_provider
        .persist()
        .await
        .map_err(|err| AcceptError::Store(err.to_string()))?;

    Ok(resp)
}

pub async fn save_transfer(
    sk: &str,
    request: SaveTransferRequest,
    _: RgbAcceptOptions,
) -> Result<RgbTransferItem, AcceptError> {
    if let Err(err) = request.validate() {
        let errors = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
        return Err(AcceptError::Validation(errors));
    }

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| AcceptError::Explorer(err.to_string()))?;

    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| AcceptError::Explorer(err.to_string()))?;

    let (mut rgb_account_provider, mut rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| AcceptError::Explorer(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| AcceptError::Stock(err.to_string()))?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| AcceptError::Wallet(err.to_string()))?;

    let SaveTransferRequest { consignment, .. } = request.clone();

    let transfer = Transfer::from_str(&consignment.clone())
        .map_err(|op| AcceptError::WrongConsig(op.to_string()))?;

    let (txid, transfer) =
        extract_transfer(&transfer).map_err(|err| AcceptError::WrongTransfer(err.to_string()))?;
    prefetch_transfer(transfer, &mut rgb_resolver).await?;

    let tx_status = rgb_resolver.tx_status(&txid).await?;

    let contract_id = transfer.contract_id();
    let resp = if let Some(contract) = rgb_account.contract_by(contract_id) {
        let iface = contract.contract_type().to_string();
        let contract = contract_id.to_string();

        let transfer_item = TransferItem::with(transfer, false);
        let transfer_in = TransferIn::with(
            String::new(),
            txid.to_string(),
            tx_status.into(),
            Some(transfer_item),
            None,
        );

        rgb_account.add_incoming(transfer.clone());
        RgbTransferItem::incoming(&contract, &iface, transfer_in)
    } else {
        return Err(AcceptError::NoContract(contract_id.to_string()));
    };

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| AcceptError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| AcceptError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| AcceptError::Store(err.to_string()))?;

    rgb_stock_provider
        .persist()
        .await
        .map_err(|err| AcceptError::Store(err.to_string()))?;

    Ok(resp)
}

pub async fn list_transfers(sk: &str) -> Result<Vec<RgbTransferItem>, RgbAccountError> {
    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let (mut rgb_account_provider, mut rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;

    let transfers = list_transfer_items(rgb_account.clone_no_persistence()).await?;

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    save_account_and_stock(&mut rgb_account_provider, &mut rgb_stock_provider)
        .await
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    Ok(transfers)
}

pub async fn verify_transfers(
    sk: &str,
    proxy: bool,
) -> Result<Vec<RgbTransferItem>, RgbAccountError> {
    // 1. Initialization
    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..Default::default()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let network: Network = bp_network_from_str(&get_network().await)
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(RgbAccountError::IO)?;
    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;
    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(RgbAccountError::IO)?;
    rgb_account_provider
        .fetch()
        .await
        .map_err(RgbAccountError::IO)?;
    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;

    // 2. Proxy Check for Incoming Transfers
    if proxy {
        let open_invoices = rgb_account.invoices_by(FilterStatus::new(Status::Open));
        debug!(format!(
            "[DEBUG] Checking proxy for open invoices: {:?}",
            open_invoices.len()
        ));

        for RgbInvoiceItem {
            invoice: rgb_invoice,
            seal: receipt_id,
            ..
        } in open_invoices
        {
            if let Ok(Some((consignment_str, _))) = get_consignment_ex(&receipt_id).await {
                debug!("Found consignment for {receipt_id}, processing immediately...");
                let rgb_transfer = Transfer::from_str(&consignment_str)
                    .map_err(|err| RgbAccountError::WrongTransfer(err.to_string()))?;
                let info = RgbTransferInfo::with(rgb_transfer, rgb_invoice);
                rgb_account.update_transfer(info);
            }
        }
    }

    // 3. Unified Transfer Processing Loop
    let all_transfers = rgb_account.transfers_by(FilterIncludeAll);
    for transfer_item in all_transfers {
        let transfer_armor = match &transfer_item.direction {
            Direction::In(rgb_in) => rgb_in.transfer.clone(),
            Direction::Out(rgb_out) => Some(rgb_out.transfer.clone()),
        };

        if let Some(armor) = transfer_armor {
            let rgb_transfer = Transfer::from_str(&armor)
                .map_err(|err| RgbAccountError::WrongTransfer(err.to_string()))?;

            // Update on-chain status for all transfers
            rgb_account =
                update_transfer_chain_status(&rgb_transfer, rgb_account, &mut rgb_resolver)
                    .await
                    .map_err(RgbAccountError::Explorer)?;

            // Diagnostic: attempt to correlate this transfer with a swap PSBT, enumerate spent inputs,
            // and detect which contracts are anchored on those inputs and what "extra" shards were declared.
            {
                let this_cons_id = rgb_transfer.consignment_id();

                // Try to locate a matching swap for offers
                let mut found_psbt_b64: Option<String> = None;
                let mut found_extra_armor: Option<String> = None;
                let offers = rgb_account.offers_by(FilterIncludeAll);
                for off in offers {
                    if let Some(sw) = off.swap()
                        && sw.transfers.iter().any(|(cid, _)| *cid == this_cons_id)
                    {
                        found_psbt_b64 = Some(sw.psbt.clone());
                        if let Some(suppl) = off.suppl() {
                            found_extra_armor = suppl.extra.clone();
                        }
                        debug!(format!(
                            "verify_transfers.match swap via offer id={} consignment_id={}",
                            off.id().to_string(),
                            this_cons_id
                        ));
                        break;
                    }
                }

                // If not found in offers, try bids
                if found_psbt_b64.is_none() {
                    let bids = rgb_account.bids_by(FilterIncludeAll);
                    for bid in bids {
                        if let Some(sw) = bid.swap()
                            && sw.transfers.iter().any(|(cid, _)| *cid == this_cons_id)
                        {
                            found_psbt_b64 = Some(sw.psbt.clone());
                            if let Some(suppl) = bid.suppl() {
                                found_extra_armor = suppl.extra.clone();
                            }
                            debug!(format!(
                                "verify_transfers.match swap via bid id={} consignment_id={}",
                                bid.id().to_string(),
                                this_cons_id
                            ));
                            break;
                        }
                    }
                }

                // Parse and log extras declared for this side (if any)
                {
                    let has_extra = found_extra_armor.is_some();
                    if let Some(extra_str) = &found_extra_armor
                        && let Ok(serialized) = Vec::<u8>::from_ascii_armored_str(extra_str)
                    {
                        let confined = Confined::from_slice_checked(serialized.as_slice());
                        if let Ok(rgb_shards) =
                            RgbExtraShards::from_strict_serialized::<U16>(confined)
                        {
                            let extras: Vec<String> = rgb_shards
                                .extras
                                .iter()
                                .map(|x| x.contract_id.to_string())
                                .collect();
                            debug!(format!(
                                "verify_transfers.swap_extra_shards_for_consignment={}",
                                extras.join(",")
                            ));
                        }
                    }
                    if !has_extra {
                        debug!("verify_transfers.swap_extra_shards_for_consignment=(none)");
                    }
                }

                // With PSBT available, enumerate inputs and check contracts assigning to those inputs
                if let Some(psbt_b64) = &found_psbt_b64 {
                    if let Ok(swap_psbt) = Psbt::from_base64(psbt_b64) {
                        let prev_outs: Vec<Outpoint> = swap_psbt
                            .to_unsigned_tx()
                            .inputs
                            .iter()
                            .map(|txin| txin.prev_output)
                            .collect();
                        let prev_dbg: Vec<String> =
                            prev_outs.iter().map(|o| o.to_string()).collect();
                        debug!(format!(
                            "verify_transfers.spent_prevouts_for_consignment={}",
                            prev_dbg.join(",")
                        ));

                        if let Ok(aff_contracts) = rgb_stock.contracts_assigning(
                            prev_outs
                                .iter()
                                .copied()
                                .map(crate::rgb::compat::outpoint_bp_to_bitcoin),
                        ) {
                            let aff_ids: Vec<String> =
                                aff_contracts.map(|id| id.to_string()).collect();
                            debug!(format!(
                                "verify_transfers.contracts_assigning_spent_prevouts={}",
                                aff_ids.join(",")
                            ));
                        } else {
                            debug!("verify_transfers.contracts_assigning_spent_prevouts=(error)");
                        }
                    } else {
                        debug!("verify_transfers.swap_psbt_parse=(error)");
                    }
                } else {
                    debug!("verify_transfers.swap_psbt=(not found for this consignment)");
                }
            }

            // If mined, process based on direction
            if rgb_transfer.is_mined(&rgb_account) {
                if transfer_item.is_out() && transfer_item.is_open() {
                    // Finalize outgoing transfers
                    rgb_account.finalize_outgoing(rgb_transfer.consignment_id());
                    debug!(format!(
                        "Finalized outgoing transfer: {:?}",
                        rgb_transfer.consignment_id()
                    ));
                } else if !transfer_item.is_out() && transfer_item.is_open() {
                    // Accept incoming transfers
                    process_transfer_status_and_accept(
                        &rgb_transfer,
                        &mut rgb_account,
                        &mut rgb_stock,
                        &mut rgb_resolver,
                    )
                    .await
                    .map_err(RgbAccountError::Accept)?;
                }
            }
        }
    }

    // 4. Process Swap Supplements
    process_swap_supplements(
        &mut rgb_account,
        &mut rgb_stock,
        &mut rgb_stock_provider,
        &mut rgb_account_provider,
        &mut rgb_resolver,
    )
    .await
    .map_err(RgbAccountError::Swap)?;

    // Re-index wallet after adding tapret commits to update cache with correct Party beneficiaries
    // This prevents cache inconsistency panics when tapret-tweaked outputs are in cache.utxo with Party::Unknown

    // Log cache state BEFORE re-indexing to diagnose Party issues
    debug!(format!(
        "verify_transfers.cache_debug.before_reindex utxo_count={} tx_count={}",
        rgb_account.wallet.cache_l1().utxo.len(),
        rgb_account.wallet.cache_l1().tx.len()
    ));

    // Log each UTXO and its Party beneficiary to identify which ones would cause panic
    for outpoint in rgb_account.wallet.cache_l1().utxo.iter() {
        if let Some(tx) = rgb_account.wallet.cache_l1().tx.get(&outpoint.txid)
            && let Some(out) = tx.outputs.get(outpoint.vout_usize())
        {
            let party_type = match &out.beneficiary {
                Party::Wallet(addr) => format!("Wallet({})", addr),
                Party::Unknown(spk) => format!("Unknown({:?})", spk),
                Party::Counterparty(addr) => format!("Counterparty({})", addr),
                Party::Subsidy => "Subsidy".to_string(),
            };
            debug!(format!(
                "verify_transfers.cache_debug.utxo_before outpoint={} party={} value={}",
                outpoint, party_type, out.value
            ));
        }
    }

    debug!("verify_transfers.reindex_after_tapret_commits.start");
    let rgb_account_for_reindex = rgb_account.clone_no_persistence();

    // Apply lookahead hints BEFORE fetch to ensure all addresses are derived
    // This prevents UTXOs at high indices from being classified as Party::Unknown
    {
        let addr_net = AddressNetwork::from(network);
        let hints = rgb_account.lookahead_hints().clone();
        for (kc, idx) in hints.iter() {
            let last_known = rgb_account_for_reindex.wallet.last_derivation_index(*kc);
            let start = NormalIndex::normal(0);
            let end = if *idx >= last_known { *idx } else { last_known };
            let mut it = rgb_account_for_reindex
                .wallet
                .derive_address_range(addr_net, *kc, start, end);
            while it.next().is_some() {}
            debug!(format!(
                "verify_transfers.reindex.lookahead_applied keychain={:?} start={:?} end={:?}",
                kc, start, end
            ));
        }
    }

    rgb_account_for_reindex
        .fetch(&mut rgb_account_provider, false)
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    // Reload ONLY the wallet to get updated cache with correct Party beneficiaries from provider.kv
    // fetch() has updated rgb_account_provider.kv with the re-indexed cache
    // BPWallet::load() will read from provider.kv (not IndexedDB) in WASM
    // Do NOT reload the entire account, as that would discard in-memory transfer status updates
    rgb_account.wallet = BPWallet::get(rgb_account_provider.clone_no_persistence())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;
    debug!("verify_transfers.reindex_after_tapret_commits.complete");

    // Log cache state AFTER re-indexing to verify Party beneficiaries are corrected
    debug!(format!(
        "verify_transfers.cache_debug.after_reindex utxo_count={} tx_count={}",
        rgb_account.wallet.cache_l1().utxo.len(),
        rgb_account.wallet.cache_l1().tx.len()
    ));

    for outpoint in rgb_account.wallet.cache_l1().utxo.iter() {
        if let Some(tx) = rgb_account.wallet.cache_l1().tx.get(&outpoint.txid)
            && let Some(out) = tx.outputs.get(outpoint.vout_usize())
        {
            let party_type = match &out.beneficiary {
                Party::Wallet(addr) => format!("Wallet({})", addr),
                Party::Unknown(spk) => format!("Unknown({:?})", spk),
                Party::Counterparty(addr) => format!("Counterparty({})", addr),
                Party::Subsidy => "Subsidy".to_string(),
            };
            debug!(format!(
                "verify_transfers.cache_debug.utxo_after outpoint={} party={} value={}",
                outpoint, party_type, out.value
            ));
        }
    }

    let stock_contracts_before = match rgb_stock.contracts() {
        Ok(it) => it.count(),
        Err(_) => 0,
    };
    let account_contracts_before = rgb_account.info().contract_kits.len();
    debug!(format!(
        "verify_transfers.before_persist stock_contracts={} account_contracts={}",
        stock_contracts_before, account_contracts_before
    ));

    let pending_hosts_count = rgb_account.pending_hosts().len();
    let lookahead_hints_count = rgb_account.lookahead_hints().len();
    debug!(format!(
        "verify_transfers.before_sync pending_hosts_count={} lookahead_hints_count={}",
        pending_hosts_count, lookahead_hints_count
    ));

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;
    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    // Persist updated kv map to IndexedDB so watcher gets fresh data with correct Party beneficiaries
    // Without this, watcher loads stale cache from IndexedDB causing "cache data inconsistency" panic
    rgb_account_provider
        .persist()
        .await
        .map_err(RgbAccountError::IO)?;
    rgb_stock_provider
        .persist()
        .await
        .map_err(RgbAccountError::IO)?;

    debug!(format!(
        "verify_transfers.after_persist pending_hosts_count={} lookahead_hints_count={}",
        rgb_account.pending_hosts().len(),
        rgb_account.lookahead_hints().len()
    ));

    // Sync BTC wallet independently from blockchain after RGB operations
    // BTC wallet uses keychain 0/1 (WPKH), RGB wallet uses keychain 10 (Taproot)
    // They must maintain separate caches for their respective address spaces
    debug!("verify_transfers.sync_btc_wallet.start");

    let mut btc_account_provider =
        AccountCrossStore::btc(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    // Fetch existing data from IndexedDB
    btc_account_provider
        .fetch()
        .await
        .map_err(RgbAccountError::IO)?;

    // Get BTC account - will create fresh wallet with empty cache since we removed the old one
    let btc_account = BtcAccount::get(btc_account_provider.clone())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;

    // Re-fetch from blockchain to rebuild cache with only keychain 0/1 UTXOs
    // This includes BTC payments received from swaps to WPKH addresses
    // The fetch() method will:
    // 1. Scan blockchain for UTXOs on keychain 0/1 only (recover=false)
    // 2. Build fresh cache with correct Party::Wallet classifications
    // 3. Store the clean cache back to provider.kv via provider.set(&cache)
    btc_account
        .fetch(&mut btc_account_provider, false)
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    // Persist the clean cache to IndexedDB for future sessions
    // This ensures later calls to get_btc_wallet() will load the clean cache
    // without any keychain 10 UTXOs
    btc_account_provider
        .persist()
        .await
        .map_err(RgbAccountError::IO)?;

    debug!("verify_transfers.sync_btc_wallet.complete");

    let stock_contracts_after = match rgb_stock.contracts() {
        Ok(it) => it.count(),
        Err(_) => 0,
    };
    let account_contracts_after = rgb_account.info().contract_kits.len();
    debug!(format!(
        "verify_transfers.after_persist stock_contracts={} account_contracts={}",
        stock_contracts_after, account_contracts_after
    ));

    // 6. Return updated list
    let transfers = list_transfer_items(rgb_account.clone_no_persistence()).await?;
    Ok(transfers)
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum RgbAccountError {
    /// some request data is invalid. {0:?}
    Validation(BTreeMap<String, String>),
    /// I/O or connectivity error. {0}
    IO(CrossStoreError),
    /// retrieve server data error. {0}
    Server(ServerErrors),
    /// import contract fail. {0}
    Import(String),
    /// accept contract fail. {0}
    Accept(String),
    /// export contract fail. {0}
    Export(ExportContractError),
    /// account sync error. {0}
    Wallet(String),
    /// transfers sync error. {0}
    Transfers(String),
    /// swap sync error. {0}
    Swap(String),
    /// rgb stock fail. {0}
    Stock(String),
    /// rgb store fail. {0}
    Store(String),
    /// rgb proxy fail. {0}
    Proxy(ProxyError),
    /// block esplorer sync error. {0}
    Explorer(String),
    /// parse network error
    WrongNetwork,
    /// parse wrong transfer. {0}
    WrongTransfer(String),
    /// psbt parse error. {0}
    PsbtError(#[from] bpstd::PsbtParseError),
    /// parse tx out error.
    ParseTxoutError,
    /// terminal derivation error.
    TerminalDerivationError,
    /// parse consig error. {0}
    ArmorError(#[from] armor::StrictArmorError),
    /// rgb psbt error. {0}
    RgbPsbtError(#[from] psrgbt::RgbPsbtError),
    /// invalid fascia data. {0}
    InvalidFasciaData,
    /// prefetch error. {0}
    PrefetchError(#[from] prefetch::PrefetchError),
}

pub async fn get_wallet_data(sk: &str) -> Result<WatcherWalletResponse, RgbAccountError> {
    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(RgbAccountError::IO)?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;

    // Apply durable lookahead hints on load to avoid gap-limit blindness
    {
        let addr_net = AddressNetwork::from(network);
        let hints = rgb_account.lookahead_hints().clone();
        for (kc, idx) in hints.iter() {
            let last_known = rgb_account.wallet.last_derivation_index(*kc);
            let start = NormalIndex::normal(0);
            let end = if *idx >= last_known { *idx } else { last_known };
            let mut it = rgb_account
                .wallet
                .derive_address_range(addr_net, *kc, start, end);
            while it.next().is_some() {}
            debug!(format!(
                "wallet.lookahead.applied keychain={:?} start={:?} end={:?}",
                kc, start, end
            ));
        }
    }

    let mut contracts = vec![];
    let contracts_list: Vec<rgbstd::info::ContractInfo> = rgb_stock
        .contracts()
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?
        .collect();

    for contract in contracts_list {
        let contract_exported = get_rgb_contract(
            contract.id,
            &mut rgb_stock,
            &mut rgb_account,
            &mut rgb_resolver,
        )
        .await
        .map_err(RgbAccountError::Export)?;

        contracts.push(contract_exported);
    }

    let mut invoices = vec![];
    rgb_account
        .invoices_by(FilterIncludeAll)
        .into_iter()
        .for_each(
            |RgbInvoiceItem {
                 invoice: rgb_invoice,
                 payed: status,
                 ..
             }| {
                if let Some(contract_id) = rgb_invoice.contract
                    && let Some(contract_info) = rgb_account.contract_by(contract_id)
                {
                    let item = RgbInvoiceStatusResponse::new(
                        &rgb_invoice,
                        &status,
                        contract_info.name(),
                        contract_info.ticker(),
                        contract_info.precision(),
                    );
                    invoices.push(item);
                }
            },
        );

    let transfers = list_transfer_items(rgb_account.clone_no_persistence()).await?;
    let offers = rgb_account
        .offers_by(FilterIncludeAll)
        .iter()
        .cloned()
        .map(|x| x.into())
        .collect();
    let bids = rgb_account
        .bids_by(FilterIncludeAll)
        .iter()
        .cloned()
        .map(|x| x.into())
        .collect();

    let rgb_keychain = AssetType::RGB20.to_keychain();
    let address = rgb_account.wallet.next_address(rgb_keychain, false);
    // Include both primary (segwit) and alt (taproot) keychains so Taproot change is visible
    let rgb_keychains = [
        AssetType::RGB20.to_keychain(),
        AssetType::RGB20.to_alt_keychain(),
    ];

    let rgb_wallet = BPWallet::get(rgb_account_provider.clone())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;

    let mut utxos: Vec<_> = rgb_wallet
        .utxos()
        .filter(|x| rgb_keychains.contains(&x.terminal.keychain))
        .collect();

    utxos.sort_by_key(|a| std::cmp::Reverse(a.value));

    let client = rgb_resolver.clone().explorer().expect("esplora client");
    let mut next_outpoint = none!();
    let mut next_outpoint_value = none!();

    for utxo in utxos {
        let Outpoint { txid, vout: index } = utxo.outpoint;
        if let Some(outpoint) = client
            .output_status(&txid, index.into_u32().into())
            .await
            .expect("esplora output status")
            && !outpoint.spent
        {
            next_outpoint = Some(utxo.outpoint.to_string());
            next_outpoint_value = Some(utxo.value.into());
            break;
        }
    }

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    save_account_and_stock(&mut rgb_account_provider, &mut rgb_stock_provider)
        .await
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    Ok(WatcherWalletResponse {
        next_utxo: next_outpoint.map(|x| x.to_string()),
        next_utxo_value: next_outpoint_value,
        next_address: Some(address.to_string()),
        list_contracts: contracts,
        transfers,
        invoices,
        bids,
        offers,
    })
}

pub async fn get_contract(sk: &str, contract: String) -> Result<ContractResponse, RgbAccountError> {
    if contract.is_empty() {
        let errors = bmap! { "contract".to_string() => "parameter is required".to_string() };
        return Err(RgbAccountError::Validation(errors));
    }

    let contract = ContractId::from_str(&contract).map_err(|_| {
        RgbAccountError::Validation(
            bmap! { "contract_id".to_string() => "parse error".to_string() },
        )
    })?;

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    bp_network_from_str(&get_network().await)
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(RgbAccountError::IO)?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;

    let contract_exported = get_rgb_contract(
        contract,
        &mut rgb_stock,
        &mut rgb_account,
        &mut rgb_resolver,
    )
    .await
    .map_err(RgbAccountError::Export)?;

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_stock_provider
        .persist()
        .await
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    Ok(contract_exported)
}

pub async fn get_contract_media(
    sk: &str,
    contract: String,
) -> Result<ContractResponse, RgbAccountError> {
    if contract.is_empty() {
        let errors = bmap! { "contract".to_string() => "parameter is required".to_string() };
        return Err(RgbAccountError::Validation(errors));
    }

    let contract = ContractId::from_str(&contract).map_err(|_| {
        RgbAccountError::Validation(
            bmap! { "contract_id".to_string() => "parse error".to_string() },
        )
    })?;

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    bp_network_from_str(&get_network().await)
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(RgbAccountError::IO)?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;

    let mut contract_exported = get_rgb_contract(
        contract,
        &mut rgb_stock,
        &mut rgb_account,
        &mut rgb_resolver,
    )
    .await
    .map_err(RgbAccountError::Export)?;

    let is_uda = contract_exported.iface == "RGB21";
    if is_uda && let Some(media) = contract_exported.meta {
        contract_exported.meta = Some(
            get_proxy_media(media)
                .await
                .map_err(RgbAccountError::Proxy)?,
        );
    }

    Ok(contract_exported)
}

pub async fn import_contract(
    sk: &str,
    contract: String,
) -> Result<ContractResponse, RgbAccountError> {
    if contract.is_empty() {
        let errors = bmap! { "contract".to_string() => "parameter is required".to_string() };
        return Err(RgbAccountError::Validation(errors));
    }

    let contract = Contract::from_str(&contract).map_err(|_| {
        RgbAccountError::Validation(bmap! { "contract".to_string() => "parse error".to_string() })
    })?;

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    bp_network_from_str(&get_network().await)
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(RgbAccountError::IO)?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;

    let type_system = AssetSchema::from(contract.schema.schema_id()).types();
    let contract = contract
        .validate(
            &rgb_resolver,
            &crate::rgb::compat::validation_config(rgb_account.chain_net(), type_system),
        )
        .map_err(|err| {
            RgbAccountError::Validation(
                bmap! { "contract".to_string() => crate::rgb::compat::validation_error_text(err) },
            )
        })?;

    rgb_stock
        .import_contract(contract.clone(), rgb_resolver.clone())
        .map_err(|op| RgbAccountError::Import(op.to_string()))?;

    let contract_exported = get_rgb_contract(
        contract.contract_id(),
        &mut rgb_stock,
        &mut rgb_account,
        &mut rgb_resolver,
    )
    .await
    .map_err(RgbAccountError::Export)?;

    let contract_info = ContractInfo::with(contract_exported.clone());
    rgb_account.add_contract(contract_info);

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_stock_provider
        .persist()
        .await
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    Ok(contract_exported)
}

pub async fn list_contracts(
    sk: &str,
    meta: bool,
) -> Result<Vec<ContractResponse>, RgbAccountError> {
    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    bp_network_from_str(&get_network().await)
        .map_err(|err| RgbAccountError::Explorer(err.to_string()))?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(RgbAccountError::IO)?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(RgbAccountError::IO)?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| RgbAccountError::Wallet(err.to_string()))?;

    let mut contracts = vec![];
    let contracts_list: Vec<rgbstd::info::ContractInfo> = rgb_stock
        .contracts()
        .map_err(|err| RgbAccountError::Stock(err.to_string()))?
        .collect();

    for contract in contracts_list {
        let mut contract_exported = get_rgb_contract(
            contract.id,
            &mut rgb_stock,
            &mut rgb_account,
            &mut rgb_resolver,
        )
        .await
        .map_err(RgbAccountError::Export)?;

        let is_uda = contract_exported.iface == "RGB21";
        if is_uda
            && meta
            && let Some(media) = contract_exported.meta
        {
            contract_exported.meta = Some(
                get_proxy_media(media)
                    .await
                    .map_err(RgbAccountError::Proxy)?,
            );
        }

        contracts.push(contract_exported);
    }

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    rgb_stock_provider
        .persist()
        .await
        .map_err(|err| RgbAccountError::Store(err.to_string()))?;

    Ok(contracts)
}

#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum OrderErrors {
    /// retrieve offers error. {0}
    IO(CrossStoreError),
    /// rgb indexer error. {0}
    Explorer(String),
    /// rgb stock error. {0}
    Store(String),
    /// retrieve server data error. {0}
    Server(ServerErrors),
    /// contract export fail. {0}
    Export(ExportContractError),
    /// offer {0} not found.
    NoOffer(String),
    /// bid {0} not found.
    NoBid(String),
    /// not enough funds to pay fee of {fee} sats; all inputs in rgb vault contain {input_value} sats.
    NoFundsForFee { input_value: Sats, fee: Sats },
    /// wrong network
    WrongNetwork,
    /// wrong contract {0}
    WrongContract(String),
    /// create offer fail. {0}
    InvalidOffer(String),
    /// create bid fail. {0}
    InvalidBid(String),
    /// create swap fail. {0}
    InvalidSwap(String),
    /// sign partial psbt fail. {0}
    InvalidSign(String),
    /// broadcast swap psbt fail. {0}
    InvalidPsbt(String),
    /// broadcast proxy data fail. {0}
    InvalidProxy(String),
}

pub async fn public_offers(_sk: &str) -> Result<Vec<RgbOfferResponse>, OrderErrors> {
    let pub_offers = list_public_offers().await.map_err(OrderErrors::Server)?;
    let pub_offers = pub_offers
        .into_iter()
        .filter(|x| x.is_open())
        .map(|x| x.into())
        .collect();

    Ok(pub_offers)
}

pub async fn offers(sk: &str) -> Result<Vec<RgbOfferResponse>, OrderErrors> {
    let network =
        bp_network_from_str(&get_network().await).map_err(|_| OrderErrors::WrongNetwork)?;

    let (rgb_account_provider, rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut my = vec![];
    let offers = rgb_account.offers_by(FilterIncludeAll);
    for offer in offers {
        if let Ok(contract_info) = rgb_stock.export_contract(offer.contract_id()) {
            my.push(offer.to_public(format!("{contract_info}")).into());
        }
    }
    Ok(my)
}

pub async fn offer_swap_suppl(
    sk: &str,
    offer_id: &str,
) -> Result<Option<SwapSupplement>, OrderErrors> {
    let network =
        bp_network_from_str(&get_network().await).map_err(|_| OrderErrors::WrongNetwork)?;

    let (rgb_account_provider, _) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let filter = OrderBy::new(offer_id);
    let offers = rgb_account.offers_by(filter);

    if offers.is_empty() {
        return Err(OrderErrors::NoOffer(offer_id.to_string()));
    }

    let public_offers = list_public_offers().await.map_err(OrderErrors::Server)?;
    let suppl = if let Some(public_offer) = public_offers.iter().find(|x| x.id == *offers[0].id()) {
        public_offer.swap_suppl.clone()
    } else {
        none!()
    };
    Ok(suppl)
}

pub async fn bid_swap_suppl(sk: &str, bid_id: &str) -> Result<Option<SwapSupplement>, OrderErrors> {
    let network =
        bp_network_from_str(&get_network().await).map_err(|_| OrderErrors::WrongNetwork)?;

    let (rgb_account_provider, _) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let filter = OrderBy::new(bid_id);
    let bids = rgb_account.bids_by(filter);

    if bids.is_empty() {
        return Err(OrderErrors::NoBid(bid_id.to_string()));
    }

    let bid = bids[0].clone();
    Ok(bid.swap().clone())
}

pub async fn bids(sk: &str) -> Result<Vec<RgbBidResponse>, OrderErrors> {
    let network =
        bp_network_from_str(&get_network().await).map_err(|_| OrderErrors::WrongNetwork)?;

    let (rgb_account_provider, rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut my = vec![];
    let bids = rgb_account.bids_by(FilterIncludeAll);
    for bid in bids {
        if let Some(contract_id) = bid.contract_id() {
            if let Ok(contract_info) = rgb_stock.export_contract(contract_id) {
                my.push(bid.to_public(format!("{contract_info}")).into());
            } else {
                my.push(bid.to_public(String::new()).into());
            }
        } else {
            my.push(bid.to_public(String::new()).into());
        }
    }

    Ok(my)
}

pub async fn can_create_offers(
    sk: &str,
    request: RgbOfferRequest,
    options: OfferOptions,
) -> Result<RgbValidationResponse, SwapErrors> {
    if let Err(err) = request.validate() {
        let _errors: Vec<_> = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
    }

    let _rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| SwapErrors::Explorer(err.to_string()))?;

    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| SwapErrors::Explorer(err.to_string()))?;

    let (rgb_account_provider, rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| SwapErrors::Store(err.to_string()))?;

    let rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| SwapErrors::Store(err.to_string()))?;

    let rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| SwapErrors::Store(err.to_string()))?;

    let rgb_wallet = rgb_account.wallet.clone_no_persistence();

    let mut result = RgbValidationResponse::default();

    for RgbOfferItem {
        contract_id: contract_id_str,
        contract_amount: asset_amount,
        ..
    } in request.offers.clone()
    {
        let contract_id = match ContractId::from_str(&contract_id_str.clone()) {
            Ok(id) => id,
            Err(e) => {
                result.failure(
                    RgbValidation::InvalidContract(contract_id_str.to_string()).to_string(),
                );
                warn!(format!("contract parse error: {e:?}"));
                continue;
            }
        };

        let contract_kit = rgb_account.contract_by(contract_id);
        if contract_kit.is_none() {
            result.failure(RgbValidation::NoContract(contract_id_str.to_string()).to_string());
            warn!(format!("contract parse error: {contract_id_str}"));
            continue;
        }

        let asset_type = contract_kit.unwrap().contract_type().clone();

        let iface_name: TypeName = tn!(asset_type.to_interface_name());
        let rgb_contract = rgb_stock.contract_data(contract_id).map_err(|_e| {
            SwapErrors::ContractIfaceError(contract_id.to_string(), iface_name.to_string())
        })?;

        let rgb_keychains = [
            AssetType::RGB20.to_keychain(),
            AssetType::RGB21.to_alt_keychain(),
        ];

        let utxos: Vec<_> = rgb_wallet.coinselect(Sats::ZERO, |x| matches!(x.status, TxStatus::Mined(_) if rgb_keychains.contains(&x.terminal.keychain))).collect();
        if utxos.is_empty() {
            result.failure(RgbValidation::NoFundVault.to_string());
            warn!(format!(
                "rgb validation error: no fund vault {contract_id_str}"
            ));
            continue;
        }

        let filter = FilterStatus::new(Status::Open);
        let open_seals = rgb_account.invoices_by(filter.clone());
        if !open_seals.is_empty() {
            let seals = open_seals.iter().map(|x| x.seal.clone()).collect();
            result.warning(RgbValidation::OpenSeals(seals).to_string());
            continue;
        }

        let pending_transfers = rgb_account.transfers_by(filter);
        if !pending_transfers.is_empty() {
            let transfers = pending_transfers
                .iter()
                .map(|x| x.transfer_id().unwrap().to_string())
                .collect::<Vec<_>>();
            result.warning(RgbValidation::OpenTransfers(transfers).to_string());
            continue;
        }

        match asset_type {
            AssetType::RGB20 => {
                let mut sum = Amount::ZERO;
                let fungible_name = FieldName::from_str("assetOwner")?;
                let fungible_amount: Amount = ContractAmount::convert(asset_amount.clone())
                    .to_value()
                    .into();

                let fungible_allocation = rgb_contract
                    .fungible(fungible_name, &FilterIncludeAll)
                    .map_err(SwapErrors::ContractError)?
                    .collect::<Vec<_>>();

                let available_utxos: Vec<_> = rgb_wallet
                    .coinselect(Sats::ZERO, |x| matches!(x.status, TxStatus::Mined(_) if x.terminal.keychain.eq(&asset_type.to_keychain()))).collect();

                let mut inputs: Vec<Utxo> = vec![];
                fungible_allocation.into_iter().for_each(|a| {
                    if let Some(outpoint) = a.seal.outpoint() {
                        let bp_out = crate::rgb::compat::outpoint_bitcoin_to_bp(outpoint);
                        if available_utxos.contains(&bp_out)
                            && fungible_amount > sum
                            && let Some((input, _)) = rgb_wallet.utxo(bp_out)
                        {
                            sum += a.state;
                            inputs.push(input);
                        }
                    }
                });

                if sum < fungible_amount {
                    let expected_value = ContractAmount::convert(asset_amount);
                    let current_amount =
                        ContractAmount::with(sum.value(), expected_value.precision);

                    result.failure(
                        RgbValidation::NoAllocation(
                            expected_value.to_string(),
                            current_amount.to_string(),
                        )
                        .to_string(),
                    );
                    continue;
                }
            }
            AssetType::RGB21 => {
                let allocation_name = FieldName::from_str("assetOwner")?;

                let uda_allocation = rgb_contract
                    .data(allocation_name, &FilterIncludeAll)
                    .map_err(SwapErrors::ContractError)?
                    .collect::<Vec<_>>();

                let avaliable_utxos: Vec<_> = rgb_wallet
                    .coinselect(Sats::ZERO, |x| matches!(x.status, TxStatus::Mined(_) if x.terminal.keychain.eq(&asset_type.to_keychain()))).collect();

                let mut allocations: u64 = 0;
                uda_allocation.into_iter().for_each(|a| {
                    let allocation: Allocation = a.clone().state.into();
                    let token_fraction: u64 = allocation.fraction().into();
                    if token_fraction > 0
                        && let Some(outpoint) = a.seal.outpoint()
                    {
                        let bp_out = crate::rgb::compat::outpoint_bitcoin_to_bp(outpoint);
                        if let Some((utxo, _)) = rgb_wallet.utxo(bp_out)
                            && avaliable_utxos.contains(&utxo.outpoint)
                        {
                            allocations += 1;
                        }
                    }
                });

                let uda_amount = ContractAmount::convert(asset_amount.clone());
                if allocations < uda_amount.int {
                    result.failure(
                        RgbValidation::NoAllocation(
                            asset_amount.to_string(),
                            allocations.to_string(),
                        )
                        .to_string(),
                    );
                    continue;
                }
            }
            _ => {}
        }

        if options.sign && options.private_keys.is_empty() {
            result.warning(RgbValidation::NoKeys.to_string());
            continue;
        }
    }

    Ok(result)
}

pub async fn create_offer(
    sk: &str,
    request: RgbOfferRequest,
    options: OfferOptions,
) -> Result<Vec<RgbOfferResponse>, OrderErrors> {
    let network =
        bp_network_from_str(&get_network().await).map_err(|_| OrderErrors::WrongNetwork)?;

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| OrderErrors::Explorer(err.to_string()))?;

    let (mut rgb_account_provider, mut rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    watcher_prefetch(sk, "bitcoin")
        .await
        .map_err(|err| OrderErrors::Explorer(err.to_string()))?;

    let mut btc_account_provider =
        AccountCrossStore::btc(sk, &get_network().await).map_err(OrderErrors::IO)?;
    btc_account_provider
        .fetch()
        .await
        .map_err(OrderErrors::IO)?;

    let mut btc_account = BtcAccount::get(btc_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let RgbOfferRequest {
        offers: new_offers, ..
    } = request.clone();

    let mut offers_created = vec![];
    for new_offer in new_offers {
        let RgbOfferItem {
            contract_id,
            contract_amount,
            counter_party,
            ..
        } = new_offer.clone();
        let contract_id = ContractId::from_str(&contract_id)
            .map_err(|_| OrderErrors::WrongContract(contract_id.to_string()))?;

        if let OrderValue::Contract(swap_kit) = counter_party.clone() {
            let counter_party =
                Contract::from_str(&swap_kit.kit.armored.clone()).map_err(|_| {
                    debug!("contract counter party invalid");
                    OrderErrors::WrongContract("contract offer invalid".to_string())
                })?;

            let type_system = AssetSchema::from(counter_party.schema_id()).types();
            let counter_party = counter_party
                .validate(
                    &rgb_resolver,
                    &crate::rgb::compat::validation_config(rgb_account.chain_net(), type_system),
                )
                .map_err(|err| {
                    OrderErrors::WrongContract(crate::rgb::compat::validation_error_text(err))
                })?;

            rgb_stock
                .import_contract(counter_party.clone(), rgb_resolver.clone())
                .map_err(|op| OrderErrors::WrongContract(op.to_string()))?;
        }

        if let Some(contract_info) = rgb_account.contract_by(contract_id) {
            let contract_exported = get_rgb_contract(
                contract_id,
                &mut rgb_stock,
                &mut rgb_account,
                &mut rgb_resolver,
            )
            .await
            .map_err(OrderErrors::Export)?;

            let mut total_allocation = 0;
            let contract_amount =
                ContractAmount::parse(contract_amount, *contract_info.precision());
            let prev_allocations: Vec<_> = contract_exported
                .allocations
                .iter()
                .filter(|x| x.is_mine && !x.is_spent)
                .filter(|x| match &x.value {
                    ContractValue::Value(fungible) => {
                        if total_allocation <= contract_amount.to_value() {
                            total_allocation += fungible;
                            true
                        } else {
                            false
                        }
                    }
                    ContractValue::UDA(alloc) => {
                        if total_allocation <= contract_amount.to_value() {
                            total_allocation += alloc.fraction;
                            true
                        } else {
                            false
                        }
                    }
                })
                .collect();

            let mut utxos_kits = set![];

            prev_allocations.iter().for_each(|x| {
                let outpoint = x.utxo.parse().expect("invalid x_output_seal");
                if let Ok((wallet_utxo, _)) = rgb_account.wallet.outpoint_by(outpoint) {
                    utxos_kits.insert(wallet_utxo.into());
                };
            });

            let prev_allocatinos: Vec<Outpoint> = contract_exported
                .allocations
                .into_iter()
                .map(|x| x.utxo.parse().expect("invalid utxo"))
                .collect();

            let utxos_kits: Vec<_> = utxos_kits.into_iter().collect();
            let contract_meta = ContractMeta::with(contract_info, utxos_kits.clone());
            let counter_party = counter_party.into();

            let mut offer_created = RgbOffer::new(contract_amount, contract_meta, counter_party);
            let mut offer_suppl = offer_created
                .create_suppl(
                    prev_allocatinos,
                    &mut btc_account,
                    &mut rgb_account,
                    &mut rgb_stock,
                )
                .map_err(|err| OrderErrors::InvalidOffer(err.to_string()))?;

            let OfferOptions {
                sign,
                private_keys: descriptors,
                ..
            } = options.clone();
            if sign {
                let mut psbt = Psbt::from_base64(&offer_suppl.psbt).expect("invalid psbt");
                let signers: Vec<_> = descriptors
                    .iter()
                    .map(|x| {
                        let interop_key = XprivBpKey::new(&x.to_string(), network)
                            .expect("invalid parse interop key");
                        MainSigner::new(
                            XprivAccount::new(*interop_key.xpriv(), interop_key.origin().clone())
                                .expect("invalid parse xpriv account"),
                        )
                    })
                    .collect();
                let mut signs = 0;
                for signer in signers.iter() {
                    signs += psbt.sign(signer).expect("Error signing btc account");
                }
                debug!(format!("offer sign: {signs}"));
                psbt.finalize(rgb_account.wallet.descriptor());

                offer_suppl.psbt = psbt.to_base64_ver(PsbtVer::V0);
                offer_created.add_suppl(offer_suppl);
            }

            rgb_account.add_offer(offer_created.clone());
            offers_created.push(offer_created);
        }
    }

    let mut new_offers = vec![];
    let mut public_offers = vec![];
    for offer in offers_created.clone() {
        if let Ok(contract_info) = rgb_stock.export_contract(offer.contract_id()) {
            contract_info.bundled_witnesses().for_each(|x| {
                println!("{:?}", x.bundle());
            });

            public_offers.push(offer.to_public(format!("{contract_info}")));
            new_offers.push(offer.into());
        }
    }
    save_public_offers(public_offers)
        .await
        .map_err(OrderErrors::Server)?;

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    btc_account
        .sync(&mut btc_account_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    save_account_and_stock(&mut rgb_account_provider, &mut rgb_stock_provider)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    debug!(format!(
        "create_offer.after_persist pending_hosts_count={} lookahead_hints_count={}",
        rgb_account.pending_hosts().len(),
        rgb_account.lookahead_hints().len()
    ));

    // VERIFY the write by immediately reading back
    let mut verification_provider = AccountCrossStore::rgb(sk, &get_network().await)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;
    verification_provider
        .fetch()
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;
    let verified_account = RgbAccount::get(verification_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    debug!(format!(
        "create_offer.verification pending_hosts_in_memory={} pending_hosts_persisted={} match={}",
        rgb_account.pending_hosts().len(),
        verified_account.pending_hosts().len(),
        rgb_account.pending_hosts().len() == verified_account.pending_hosts().len()
    ));

    if verified_account.pending_hosts().len() != rgb_account.pending_hosts().len() {
        return Err(OrderErrors::Store(format!(
            "Persistence verification failed: expected {} pending_hosts, got {}",
            rgb_account.pending_hosts().len(),
            verified_account.pending_hosts().len()
        )));
    }

    Ok(new_offers)
}

pub async fn cancel_offer(
    sk: &str,
    request: RgbCancelOffersRequest,
) -> Result<RgbSwapCancelResponse, OrderErrors> {
    let network =
        bp_network_from_str(&get_network().await).map_err(|_| OrderErrors::WrongNetwork)?;

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| OrderErrors::Explorer(err.to_string()))?;

    let (mut rgb_account_provider, mut rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let RgbCancelOffersRequest {
        offers: cancel_offers,
        ..
    } = request.clone();

    let mut orders_cancelled = RgbSwapCancelResponse::default();
    for cancel_offer in cancel_offers {
        if let Some(offer) = rgb_account.offers_by(OrderBy::new(&cancel_offer)).first() {
            let mut offer_canceled = offer.clone();
            offer_canceled.cancel();

            rgb_account.update_offer(offer_canceled.clone());
            debug!(format!("offer canceled {cancel_offer}"));

            remove_public_offer(*offer.id())
                .await
                .map_err(OrderErrors::Server)?;

            orders_cancelled.cancel_offer(cancel_offer);
        }
    }

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    save_account_and_stock(&mut rgb_account_provider, &mut rgb_stock_provider)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    Ok(orders_cancelled)
}

pub async fn can_create_bid(
    sk: &str,
    request: RgbBidRequest,
    options: BidOptions,
) -> Result<RgbValidationResponse, SwapErrors> {
    if let Err(err) = request.validate() {
        let _errors: Vec<_> = err
            .iter()
            .map(|(f, e)| (f.to_string(), e.to_string()))
            .collect();
    }

    let _rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| SwapErrors::Explorer(err.to_string()))?;

    let network = bp_network_from_str(&get_network().await)
        .map_err(|err| SwapErrors::Explorer(err.to_string()))?;

    let (rgb_account_provider, rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| SwapErrors::Store(err.to_string()))?;

    let rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| SwapErrors::Store(err.to_string()))?;

    let rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| SwapErrors::Store(err.to_string()))?;

    watcher_prefetch(sk, "bitcoin")
        .await
        .map_err(|err| SwapErrors::Explorer(err.to_string()))?;
    let mut btc_account_provider = AccountCrossStore::btc(sk, &get_network().await)
        .map_err(|e| SwapErrors::Store(e.to_string()))?;
    btc_account_provider
        .fetch()
        .await
        .map_err(|e| SwapErrors::Store(e.to_string()))?;
    let btc_account = BtcAccount::get(btc_account_provider.clone())
        .map_err(|err| SwapErrors::Store(err.to_string()))?;
    let btc_wallet = btc_account.wallet.clone_no_persistence();

    let rgb_wallet = rgb_account.wallet.clone_no_persistence();

    let mut result = RgbValidationResponse::default();

    let RgbBidRequest { offer_id, fee, .. } = request.clone();

    let offer_id = OrderId::from_hex(&offer_id).expect("invalid offer id");
    let public_offer = get_public_offer(offer_id).await;
    if public_offer.is_err() {
        result.failure(RgbValidation::NoPublicOffer(offer_id.to_string()).to_string());
        return Ok(result);
    }

    let public_offer = public_offer.unwrap();
    if public_offer.is_none() {
        result.failure(RgbValidation::NoPublicOffer(offer_id.to_string()).to_string());
        return Ok(result);
    }

    let public_offer = public_offer.unwrap();
    let PublicRgbOffer {
        asset: asset_kit,
        counter_party,
        ..
    } = public_offer.clone();

    let contract_id = *asset_kit.info.contract_id();
    if !rgb_stock
        .contracts()
        .map_err(|e| SwapErrors::Store(e.to_string()))?
        .any(|x| x.id == contract_id)
    {
        result.warning(RgbValidation::NoContract(contract_id.to_string()).to_string());
    }

    let rgb_keychains = [
        AssetType::RGB20.to_keychain(),
        AssetType::RGB21.to_alt_keychain(),
    ];

    let utxos: Vec<_> = rgb_wallet.coinselect(Sats::ZERO, |x| matches!(x.status, TxStatus::Mined(_) if rgb_keychains.contains(&x.terminal.keychain))).collect();
    if utxos.is_empty() {
        result.warning(RgbValidation::NoFundVault.to_string());
    }

    let bitcoin_keychains = [
        AssetType::Bitcoin.to_keychain(),
        AssetType::Bitcoin.to_alt_keychain(),
    ];

    let btc_sats: Sats = btc_wallet
        .utxos()
        .filter(|u| {
            matches!(u.status, TxStatus::Mined(_))
                && bitcoin_keychains.contains(&u.terminal.keychain)
        })
        .map(|u| u.value)
        .sum();

    let rgb_sats: Sats = rgb_wallet
        .utxos()
        .filter(|u| {
            matches!(u.status, TxStatus::Mined(_))
                && bitcoin_keychains.contains(&u.terminal.keychain)
        })
        .map(|u| u.value)
        .sum();

    let avaliable_sats: Sats = btc_sats + rgb_sats;

    // Diagnostics: BTC-paying source; check if rgb wallet bitcoin-keychain UTXOs will be used
    debug!(format!(
        "can_create_bid sats btc_sats={} rgb_sats={} available_sats={}",
        btc_sats, rgb_sats, avaliable_sats
    ));
    let rgb_btc_utxos: Vec<String> = rgb_wallet
        .utxos()
        .filter(|u| {
            matches!(u.status, TxStatus::Mined(_))
                && bitcoin_keychains.contains(&u.terminal.keychain)
        })
        .map(|u| format!("{}@{}", u.outpoint, u.value))
        .take(5)
        .collect();
    debug!(format!(
        "can_create_bid rgb_bitcoin_keychain_utxos_top5={}",
        rgb_btc_utxos.join(",")
    ));

    let fee = if let ChainFee::Value(fee) = fee {
        Sats::from_sats(fee)
    } else {
        result.failure(RgbValidation::NoFee.to_string());
        return Ok(result);
    };

    match counter_party {
        CounterParty::Sats(bitcoin_price) => {
            let total_bitcoin_price = Sats::from_sats(bitcoin_price) + fee;
            if avaliable_sats < total_bitcoin_price {
                result.warning(
                    RgbValidation::NoSats(
                        total_bitcoin_price.to_string(),
                        avaliable_sats.to_string(),
                    )
                    .to_string(),
                );
            }
        }
        CounterParty::Contract(_contract) => {}
    };

    if options.sign && options.private_keys.is_empty() {
        result.warning(RgbValidation::NoKeys.to_string());
    }

    if !public_offer.is_open() {
        result.failure(RgbValidation::NoOpenOffer(offer_id.to_string()).to_string());
    }

    Ok(result)
}

pub async fn create_bid(
    sk: &str,
    request: RgbBidRequest,
    options: BidOptions,
) -> Result<RgbBidResponse, OrderErrors> {
    let network =
        bp_network_from_str(&get_network().await).map_err(|_| OrderErrors::WrongNetwork)?;

    watcher_prefetch(sk, "bitcoin")
        .await
        .map_err(|err| OrderErrors::Explorer(err.to_string()))?;

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| OrderErrors::Explorer(err.to_string()))?;

    let (mut rgb_account_provider, mut rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut btc_account_provider =
        AccountCrossStore::btc(sk, &get_network().await).map_err(OrderErrors::IO)?;
    btc_account_provider
        .fetch()
        .await
        .map_err(OrderErrors::IO)?;

    let mut btc_account = BtcAccount::get(btc_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let btc_wallet: BPWallet = btc_account.wallet.clone_no_persistence();

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let RgbBidRequest { offer_id, fee, .. } = request.clone();

    let public_offers = list_public_offers().await.map_err(OrderErrors::Server)?;

    let offer_id = OrderId::from_hex(&offer_id).expect("invalid offer id");
    let public_offer = public_offers.iter().find(|x| x.id == offer_id);
    if public_offer.is_none() {
        return Err(OrderErrors::NoOffer(offer_id.to_string()));
    };
    let offer = public_offer.unwrap().clone();
    let rgb_wallet = rgb_account.wallet.clone_no_persistence();

    // Consume Contract
    let contract = Contract::from_str(&offer.asset.armored.clone()).map_err(|_| {
        debug!("contract offer invalid");
        OrderErrors::WrongContract("contract offer invalid".to_string())
    })?;

    let type_system = AssetSchema::from(contract.schema_id()).types();
    let contract = contract
        .validate(
            &rgb_resolver,
            &crate::rgb::compat::validation_config(rgb_account.chain_net(), type_system),
        )
        .map_err(|err| {
            OrderErrors::WrongContract(crate::rgb::compat::validation_error_text(err))
        })?;

    rgb_stock
        .import_contract(contract.clone(), rgb_resolver.clone())
        .map_err(|op| OrderErrors::WrongContract(op.to_string()))?;

    let contract_exported = get_rgb_contract(
        contract.contract_id(),
        &mut rgb_stock,
        &mut rgb_account,
        &mut rgb_resolver,
    )
    .await
    .map_err(|op| OrderErrors::WrongContract(op.to_string()))?;

    let contract_info = ContractInfo::with(contract_exported.clone());
    rgb_account.add_contract(contract_info);

    // Define Inputs
    let fee = if let ChainFee::Value(fee) = fee {
        Sats::from_sats(fee)
    } else {
        return Err(OrderErrors::InvalidBid(OrderSupplError::NoUtxo.to_string()));
    };

    let bid_meta = match offer.counter_party.clone() {
        CounterParty::Sats(sats) => {
            let total_sats = Sats::from(sats) + fee;
            let outpoints: Vec<_> = btc_wallet
                .coinselect(total_sats, |x| {
                    [
                        AssetType::Bitcoin.to_keychain(),
                        AssetType::Bitcoin.to_alt_keychain(),
                    ]
                    .contains(&x.terminal.keychain)
                })
                .collect();

            let mut utxos_kits = set![];
            for outpoint in outpoints {
                if let Ok((wallet_utxo, _)) = btc_wallet.outpoint_by(outpoint) {
                    let utxo_kit: UtxoKit = wallet_utxo.into();
                    utxos_kits.insert(utxo_kit);
                }
            }

            let utxo_kits: Vec<_> = utxos_kits.into_iter().collect();
            BidAsset::L1(Layer1Meta::with(sats.into(), utxo_kits))
        }
        CounterParty::Contract(contract_swap) => {
            let ContractSwap {
                kit:
                    ContractKit {
                        info: contract_info,
                        ..
                    },
                amount: contract_amount,
                ..
            } = contract_swap;

            let counter_party_id = *contract_info.contract_id();
            let contract_exported = get_rgb_contract(
                counter_party_id,
                &mut rgb_stock,
                &mut rgb_account,
                &mut rgb_resolver,
            )
            .await
            .map_err(OrderErrors::Export)?;

            let mut total_allocation = 0;
            let mut total_balance = Sats::ZERO;

            let utxos_kit: Vec<UtxoKit> = contract_exported
                .allocations
                .iter()
                .filter(|x| x.is_mine && !x.is_spent)
                .filter(|x| match &x.value {
                    ContractValue::Value(fungible) => {
                        if total_allocation <= contract_amount.to_value() {
                            total_allocation += fungible;
                            true
                        } else {
                            false
                        }
                    }
                    ContractValue::UDA(alloc) => {
                        if total_allocation <= contract_amount.to_value() {
                            total_allocation += alloc.fraction;
                            true
                        } else {
                            false
                        }
                    }
                })
                .map(|x| {
                    let outpoint = x.utxo.parse().expect("invalid outpoint");
                    let (wallet_utxo, _) =
                        rgb_wallet.outpoint_by(outpoint).expect("invalid outpoint");

                    total_balance += wallet_utxo.value;
                    wallet_utxo.into()
                })
                .collect();

            total_balance
                .checked_sub(fee)
                .ok_or(OrderErrors::NoFundsForFee {
                    input_value: total_balance,
                    fee,
                })?;

            let contract_info = rgb_account.contract_by(counter_party_id);
            BidAsset::Contract(ContractMeta::with(contract_info.unwrap(), utxos_kit))
        }
    };

    let params = NewBidParams::with(request.fee);
    let mut bid_created = RgbBid::new(offer.clone(), request.asset_price, bid_meta);
    let mut bid_suppl = bid_created
        .create_suppl(params, &mut btc_account, &mut rgb_account, &mut rgb_stock)
        .map_err(|err| OrderErrors::InvalidBid(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let PublicRgbOffer { order_suppl, .. } = offer.clone();
    if let Some(suppl) = order_suppl
        && let Some(serialized) = suppl.shard
    {
        let serialized = Vec::<u8>::from_ascii_armored_str(&serialized)
            .map_err(|err| OrderErrors::InvalidOffer(err.to_string()))?;
        let confined = Confined::from_slice_checked(serialized.as_slice());

        let RgbShard { anchors, .. } = RgbShard::from_strict_serialized::<U16>(confined)
            .map_err(|err| OrderErrors::InvalidOffer(err.to_string()))?;

        for (witness_id, anchor) in anchors {
            rgb_stock
                .consume_anchor_raw(
                    contract.contract_id(),
                    anchor,
                    crate::rgb::compat::txid_bitcoin_to_bp(witness_id),
                    &mut rgb_stock_provider,
                )
                .map_err(|err| OrderErrors::Store(err.to_string()))?;

            rgb_stock = RGBStock::get(rgb_stock_provider.clone())
                .map_err(|err| OrderErrors::Store(err.to_string()))?;
        }
    }

    let BidOptions {
        sign,
        proxy,
        private_keys: descriptors,
        ..
    } = options.clone();

    if sign {
        let mut psbt = Psbt::from_base64(&bid_suppl.psbt).expect("invalid psbt");
        let signers: Vec<_> = descriptors
            .iter()
            .map(|x| {
                let interop_key =
                    XprivBpKey::new(&x.to_string(), network).expect("invalid parse interop key");
                MainSigner::new(
                    XprivAccount::new(*interop_key.xpriv(), interop_key.origin().clone())
                        .expect("invalid parse xpriv account"),
                )
            })
            .collect();

        let mut signs = 0;
        for signer in signers.iter() {
            signs += psbt.sign(signer).expect("Error signing btc account");
        }
        debug!(format!("bid signs: {signs}"));
        let finalized = psbt.finalize(btc_wallet.descriptor());
        if finalized == 0 {
            psbt.finalize(rgb_wallet.descriptor());
        };

        bid_suppl.psbt = psbt.to_base64_ver(PsbtVer::V0);
        bid_created.add_suppl(bid_suppl.clone());
    }

    let swap = offer
        .swap(
            rgb_account.chain_net(),
            bid_created.to_public("".to_string()),
            &mut rgb_stock,
            &mut rgb_resolver,
        )
        .map_err(|err| OrderErrors::InvalidSwap(err.to_string()))?;
    // Extract txid from the finalized swap PSBT; if not present, fail
    let swap_psbt = Psbt::from_base64(&swap.psbt)
        .map_err(|_| OrderErrors::InvalidPsbt("invalid PSBT".to_string()))?;
    let swap_txid = swap_psbt.txid();

    if proxy {
        let SwapSupplement {
            psbt, transfers, ..
        } = swap.clone();
        let broadcast_req = PublishPsbtRequest { psbt };
        publish_psbt_file_bp(broadcast_req)
            .await
            .map_err(|op| OrderErrors::InvalidPsbt(op.to_string()))?;

        let rgb_invoice = bid_suppl.beneficiary.unwrap_or_default();
        let rgb_invoice = RgbInvoice::from_str(&rgb_invoice).expect("invalid rgb invoice");

        for (_, transfer) in transfers {
            let rgb_transfer = Transfer::from_str(&transfer).expect("invalid transfer");

            publish_rgb_transfer(rgb_invoice.clone(), rgb_transfer.clone(), None)
                .await
                .map_err(|op| OrderErrors::InvalidProxy(op.to_string()))?;
        }
    }

    bid_created.add_swap(swap);
    match_public_offer(bid_created.to_public(String::new()))
        .await
        .map_err(OrderErrors::Server)?;

    rgb_account.add_bid(bid_created.clone());

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    btc_account
        .sync(&mut btc_account_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    save_account_and_stock(&mut rgb_account_provider, &mut rgb_stock_provider)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    debug!(format!(
        "create_bid.after_persist pending_hosts_count={} lookahead_hints_count={}",
        rgb_account.pending_hosts().len(),
        rgb_account.lookahead_hints().len()
    ));

    // VERIFY the write by immediately reading back
    let mut verification_provider = AccountCrossStore::rgb(sk, &get_network().await)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;
    verification_provider
        .fetch()
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;
    let verified_account = RgbAccount::get(verification_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    debug!(format!(
        "create_bid.verification pending_hosts_in_memory={} pending_hosts_persisted={} match={}",
        rgb_account.pending_hosts().len(),
        verified_account.pending_hosts().len(),
        rgb_account.pending_hosts().len() == verified_account.pending_hosts().len()
    ));

    if verified_account.pending_hosts().len() != rgb_account.pending_hosts().len() {
        return Err(OrderErrors::Store(format!(
            "Persistence verification failed: expected {} pending_hosts, got {}",
            rgb_account.pending_hosts().len(),
            verified_account.pending_hosts().len()
        )));
    }

    // Build a concrete response including the swap txid
    let public_bid = bid_created.to_public(String::new());
    let counter_party = match public_bid.counter_party.clone() {
        crate::rgb::models::CounterParty::Sats(s) => crate::structs::OrderValue::Bitcoin(s),
        crate::rgb::models::CounterParty::Contract(contract_swap) => {
            crate::structs::OrderValue::Contract(crate::structs::ContractCounterParty::with(
                contract_swap.kit,
                contract_swap.amount,
            ))
        }
    };

    let resp = crate::structs::RgbBidResponse {
        id: bid_created.id().to_string(),
        offer_id: bid_created.offer_id().to_string(),
        status: bid_created.status().to_string(),
        asset: public_bid.asset,
        amount: public_bid.amount,
        counter_party,
        txid: swap_txid.to_string(),
    };

    Ok(resp)
}

pub async fn cancel_bid(
    sk: &str,
    request: RgbCancelBidsRequest,
) -> Result<RgbSwapCancelResponse, OrderErrors> {
    let network =
        bp_network_from_str(&get_network().await).map_err(|_| OrderErrors::WrongNetwork)?;

    watcher_prefetch(sk, "rgb")
        .await
        .map_err(|err| OrderErrors::Explorer(err.to_string()))?;

    let (mut rgb_account_provider, mut rgb_stock_provider) = get_account_and_stock(sk, network)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let mut rgb_account = RgbAccount::get(rgb_account_provider.clone())
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    let RgbCancelBidsRequest {
        bids: cancel_bids, ..
    } = request.clone();

    let mut orders_cancelled = RgbSwapCancelResponse::default();
    for order_id in cancel_bids {
        if let Some(bid) = rgb_account.bids_by(OrderBy::new(&order_id)).first() {
            let mut bid_canceled = bid.clone();
            bid_canceled.cancel();

            rgb_account.update_bid(bid_canceled.clone());
            debug!(format!("bid canceled {order_id}"));

            remove_public_bid(*bid.offer_id(), *bid.id())
                .await
                .map_err(OrderErrors::Server)?;

            orders_cancelled.cancel_bid(order_id);
        }
    }

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    save_account_and_stock(&mut rgb_account_provider, &mut rgb_stock_provider)
        .await
        .map_err(|err| OrderErrors::Store(err.to_string()))?;

    Ok(orders_cancelled)
}

#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum LoadWalletsError {
    /// Wallet encryption error: {0}
    Encryption(String),
    /// Wallet decryption error: {0}
    Decryption(String),
    /// Transaction error: {0}
    Transaction(String),
    /// Network error: {0}
    Network(String),
}

pub async fn load_rgb_wallets(number_of_wallets: u16) -> Result<u16, LoadWalletsError> {
    // First we need to load the main wallet
    let hash = hash_password(&SecretString("password".to_string()))
        .map_err(|e| LoadWalletsError::Encryption(e.to_string()))?;
    let mnemonic_phrase = SecretString(get_server_wallet_seed().await);
    let seed_password = SecretString("".to_string());
    let encrypted_descriptors = encrypt_wallet(&mnemonic_phrase, &hash, &seed_password)
        .await
        .map_err(|e| LoadWalletsError::Encryption(e.to_string()))?;
    let vault = decrypt_wallet(&hash, &encrypted_descriptors, &seed_password)
        .await
        .map_err(|e| LoadWalletsError::Decryption(e.to_string()))?;

    let mut created = 0;
    for i in 1..=number_of_wallets {
        let local_seed_password = SecretString(format!("goat{}", i));
        let local_encrypted_descriptors =
            encrypt_wallet(&mnemonic_phrase, &hash, &local_seed_password)
                .await
                .map_err(|e| LoadWalletsError::Encryption(e.to_string()))?;
        decrypt_wallet(&hash, &local_encrypted_descriptors, &local_seed_password)
            .await
            .map_err(|e| LoadWalletsError::Decryption(e.to_string()))?;

        let wallet_data = get_wallet_data(&vault.private.nostr_prv)
            .await
            .map_err(|e| LoadWalletsError::Network(e.to_string()))?;

        let local_address = wallet_data
            .next_address
            .ok_or_else(|| LoadWalletsError::Network("No address available".to_string()))?;

        // Send sats to the new wallet
        send_sats(
            &SecretString(vault.private.btc_descriptor_xprv.clone()),
            &SecretString(vault.private.btc_change_descriptor_xprv.clone()),
            &local_address,
            50000,
            true,
            None,
        )
        .await
        .map_err(|e| LoadWalletsError::Transaction(e.to_string()))?;

        created += 1;
    }

    Ok(created)
}

#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum AtomicErrors {
    IO(CrossStoreError),
    /// retrieve server data error. {0}
    Server(ServerErrors),
    /// rgb stock fail. {0}
    Stock(String),
    /// rgb store fail. {0}
    Store(String),
    /// rgb proxy fail. {0}
    Proxy(ProxyError),
    /// block esplorer sync error. {0}
    Explorer(String),
    /// Keys error occurred: {0}
    Keys(String),
    /// Encode error occurred: {0}
    Encode(String),
    /// Watcher error occurred: {0}
    Watcher(String),
    /// Issue error occurred: {0}
    Issue(IssueError),
    /// Order error occurred: {0}
    Order(OrderErrors),
    /// Refresh error occurred: {0}
    Refresh(String),
    WrongAsset(String),
}

pub async fn atomic_stats() -> Result<AtomicStatsResponse, AtomicErrors> {
    let server_hash = hash_password(&SecretString("password".to_string()))
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_seed = SecretString(get_server_wallet_seed().await);
    let server_pass = SecretString("".to_string());
    let encrypt = encrypt_wallet(&server_seed, &server_hash, &server_pass)
        .await
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_vault = decrypt_wallet(&server_hash, &encrypt, &server_pass)
        .await
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let sk = server_vault.private.nostr_prv.clone();
    watcher_prefetch(&sk, "rgb")
        .await
        .map_err(|err| AtomicErrors::Explorer(err.to_string()))?;

    bp_network_from_str(&get_network().await)
        .map_err(|err| AtomicErrors::Explorer(err.to_string()))?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(&sk, &get_network().await).map_err(AtomicErrors::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| AtomicErrors::Stock(err.to_string()))?;

    let rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| AtomicErrors::Stock(err.to_string()))?;

    let contracts: Vec<_> = rgb_stock
        .contracts()
        .map_err(|err| AtomicErrors::Stock(err.to_string()))?
        .map(|x| x.id.to_string())
        .collect();

    let iface_or_coin = "RGB21";

    // UTXOss
    let free_slot = watcher_unspent_utxos(&sk, iface_or_coin, true)
        .await
        .map_err(|x| AtomicErrors::Watcher(x.to_string()))?;

    let locked_slot = watcher_anchored_utxos(&sk, iface_or_coin)
        .await
        .map_err(|x| AtomicErrors::Watcher(x.to_string()))?;

    // Address
    let addresses = watcher_next_addresses(&sk, iface_or_coin, 10)
        .await
        .map_err(|x| AtomicErrors::Watcher(x.to_string()))?;
    let addresses = addresses.iter().map(|x| x.address.clone()).collect();

    Ok(AtomicStatsResponse {
        contracts,
        slots_free: free_slot,
        slots_locked: locked_slot,
        addresses,
    })
}

pub async fn atomic_load() -> Result<WatcherWalletResponse, AtomicErrors> {
    let server_hash = hash_password(&SecretString("password".to_string()))
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_seed = SecretString(get_server_wallet_seed().await);
    let server_pass = SecretString("".to_string());
    let encrypt = encrypt_wallet(&server_seed, &server_hash, &server_pass)
        .await
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_vault = decrypt_wallet(&server_hash, &encrypt, &server_pass)
        .await
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_sk = server_vault.private.nostr_prv.clone();
    let resp = get_wallet_data(&server_sk)
        .await
        .map_err(|err| AtomicErrors::Watcher(err.to_string()))?;

    Ok(resp)
}

pub async fn atomic_refresh() -> Result<(), AtomicErrors> {
    let server_hash = hash_password(&SecretString("password".to_string()))
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_seed = SecretString(get_server_wallet_seed().await);
    let server_pass = SecretString("".to_string());
    let encrypt = encrypt_wallet(&server_seed, &server_hash, &server_pass)
        .await
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_vault = decrypt_wallet(&server_hash, &encrypt, &server_pass)
        .await
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_sk = server_vault.private.nostr_prv.clone();

    verify_transfers(&server_sk, true)
        .await
        .map_err(|err| AtomicErrors::Refresh(err.to_string()))?;

    Ok(())
}

pub async fn atomic_mint(
    request: AtomicMintRequest,
    options: IssueContractOptions,
) -> Result<AtomicMintResponse, AtomicErrors> {
    let server_hash = hash_password(&SecretString("password".to_string()))
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_seed = SecretString(get_server_wallet_seed().await);
    let server_pass = SecretString("".to_string());
    let encrypt = encrypt_wallet(&server_seed, &server_hash, &server_pass)
        .await
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_vault = decrypt_wallet(&server_hash, &encrypt, &server_pass)
        .await
        .map_err(|err| AtomicErrors::Encode(err.to_string()))?;

    let server_sk = server_vault.private.nostr_prv.clone();
    let server_rgb_key = server_vault.public.rgb_descriptor_xpub.clone();
    let server_btc_key = server_vault.public.btc_descriptor_xpub.clone();

    create_watcher(
        &server_sk,
        WatcherRequest {
            name: "default".to_string(),
            btc_xpub: server_btc_key,
            rgb_xpub: server_rgb_key,
            recover: false,
        },
    )
    .await
    .map_err(|err| AtomicErrors::Watcher(err.to_string()))?;

    let next_utxo = watcher_unspent_utxo(&server_sk, "rgb21", true)
        .await
        .map_err(|err| AtomicErrors::Watcher(err.to_string()))?;

    let seal = next_utxo.utxo.unwrap().clone().to_string();
    let issue_request = IssueRequest {
        supply: "1".to_string(),
        precision: 1,
        seal,
        ticker: request.ticker,
        name: request.name,
        iface: request.iface,
        description: request.description,
        chain: request.chain,
        meta: request.meta,
    };

    let contract_exported = issue_contract(&server_sk, issue_request, options)
        .await
        .map_err(AtomicErrors::Issue)?;

    let rgb_offer_request = RgbOfferRequest {
        strategy: RgbSwap::P2P,
        offers: vec![RgbOfferItem {
            contract_id: contract_exported.contract_id.clone(),
            expire_at: None,
            contract_amount: "1".to_string(),
            counter_party: OrderValue::Bitcoin(request.bitcoin_price),
        }],
    };

    let PrivateWalletData {
        btc_descriptor_xprv: ref server_btc_prv,
        btc_change_descriptor_xprv: ref server_btc_change_prv,
        rgb_descriptor_xprv: ref server_rgb_prv,
        ..
    } = server_vault.private.clone();

    let secret_keys = vec![
        SecretString(server_btc_prv.to_string()),
        SecretString(server_btc_change_prv.to_string()),
        SecretString(server_rgb_prv.to_string()),
    ];

    let swap_response = create_offer(
        &server_sk,
        rgb_offer_request,
        OfferOptions::p2p(secret_keys),
    )
    .await
    .map_err(AtomicErrors::Order)?;

    Ok(AtomicMintResponse {
        issue_response: contract_exported.clone(),
        rgb_offer_response: swap_response,
    })
}

pub fn get_rgb_version() -> String {
    RGB_VERSION.to_owned()
}

#[cfg(target_arch = "wasm32")]
pub async fn rgb_backup(sk: &str) -> Result<bool, WatcherError> {
    watcher_prefetch(sk, "rgb").await?;

    let _network =
        bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(WatcherError::IO)?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| WatcherError::Store(err.to_string()))?;

    rgb_account_provider
        .fetch()
        .await
        .map_err(WatcherError::IO)?;

    let rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| WatcherError::Store(err.to_string()))?;

    backup_rgb_stock(sk, rgb_stock)
        .await
        .map_err(WatcherError::Server)?;
    backup_rgb_account(sk, &mut rgb_account_provider)
        .await
        .map_err(WatcherError::Server)?;
    Ok(true)
}

#[cfg(target_arch = "wasm32")]
pub async fn rgb_restore(sk: &str) -> Result<bool, WatcherError> {
    let _network =
        bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(WatcherError::IO)?;

    let mut rgb_account_provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?;

    let mut rgb_stock = restore_rgb_stock(sk).await.map_err(WatcherError::Server)?;

    let mut rgb_account = restore_rgb_account(sk, &mut rgb_account_provider)
        .await
        .map_err(WatcherError::Server)?;

    rgb_account
        .sync(&mut rgb_account_provider)
        .map_err(|err| WatcherError::Store(err.to_string()))?;

    rgb_stock
        .sync(&mut rgb_stock_provider)
        .map_err(|err| WatcherError::Store(err.to_string()))?;

    rgb_account_provider
        .persist()
        .await
        .map_err(|err| WatcherError::Store(err.to_string()))?;

    rgb_stock_provider
        .persist()
        .await
        .map_err(|err| WatcherError::Store(err.to_string()))?;
    watcher_prefetch(sk, "rgb").await?;

    Ok(true)
}
