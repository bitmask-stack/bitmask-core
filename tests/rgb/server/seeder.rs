#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(not(target_arch = "wasm32"))]

use amplify::{default, none};
use bitmask_core::bitcoin::{get_mnemonic, new_mnemonic, save_mnemonic};
use bitmask_core::bp::{FundOptions, fund_vault};
use bitmask_core::regtest::{new_block, send_coins};
use bitmask_core::rgb::issue::IssueContractOptions;
use bitmask_core::rgb::models::{AssetType, ContractAmount};
use bitmask_core::rgb::transfer::{RgbInvoiceOptions, RgbTransferOptions};
use bitmask_core::rgb::{create_invoice, import_contract, issue_contract, transfer_contract};
use bitmask_core::structs::{
    ChainFee, ChainTarget, ContractResponse, DecryptedWalletData, FundVaultDetails, IssueMedia,
    IssueRequest, IssueResponse, MediaData, NextAddressResponse, NextUtxoResponse,
    PrivateWalletData, PublicWalletData, RgbInvoiceRequest, RgbInvoiceResponse, RgbInvoiceValue,
    RgbSelection, RgbTransferRequest, RgbTransferResponse, SecretString, WatcherRequest,
    WatcherResponse,
};
use bitmask_core::wallet::watcher::{
    create_watcher, watcher_next_address, watcher_next_change_address, watcher_next_utxo,
};
use bitmask_core::{debug, info};
use bpstd::Sats;
use rgbcore::ContractId;
use rgbinvoice::RgbInvoice;
use rgbstd::containers::{ConsignmentExt, Contract};
use std::str::FromStr;

pub fn get_hashed_media() -> IssueMedia {
    IssueMedia {
        media: Some(MediaData {
            ty: "image/png".to_string(),
            uri: "b9bf23a30bedcc4c7e5b7e83412f8962ed33012dbb822a15fb536e5f2fbf9d28".to_string(),
        }),
        preview: Some(MediaData {
            ty: "image/png".to_string(),
            uri: "b9bf23a30bedcc4c7e5b7e83412f8962ed33012dbb822a15fb536e5f2fbf9d28".to_string(),
        }),
        attachments: vec![MediaData {
            ty: "image/png".to_string(),
            uri: "b9bf23a30bedcc4c7e5b7e83412f8962ed33012dbb822a15fb536e5f2fbf9d28".to_string(),
        }],
    }
}

pub fn get_uri_media() -> IssueMedia {
    let media_item = MediaData {
        ty: "image/svg+xml".to_string(),
        uri: "https://bitcoin.org/img/icons/logotop.svg".to_string(),
    };

    IssueMedia {
        media: Some(media_item.clone()),
        preview: Some(media_item.clone()),
        attachments: vec![media_item],
    }
}

#[derive(Default)]
pub struct UtxoFilter {
    pub outpoint_equal: Option<String>,
    pub amount_less_than: Option<u64>,
    pub amount_equal_than: Option<u64>,
}

impl UtxoFilter {
    pub fn amount_eq(amount: u64) -> Self {
        Self {
            outpoint_equal: None,
            amount_less_than: None,
            amount_equal_than: Some(amount),
        }
    }

    pub fn amount_lt(amount: u64) -> Self {
        Self {
            outpoint_equal: None,
            amount_less_than: Some(amount),
            amount_equal_than: None,
        }
    }
}

pub struct ContractSeed {
    pub iface: String,
    pub supply: String,
    pub vault: DecryptedWalletData,
    pub filter: UtxoFilter,
    pub options: IssueContractOptions,
    pub quantity: u8,
    pub meta: Option<IssueMedia>,
}

impl ContractSeed {
    pub fn fungible(vault: DecryptedWalletData, supply: Option<ContractAmount>) -> Self {
        let options = IssueContractOptions {
            proxy: true,
            ..Default::default()
        };

        let supply = supply.map_or(ContractAmount::new(5, 10, 2).to_string(), |x| x.to_string());
        Self {
            iface: AssetType::RGB20.to_string(),
            filter: UtxoFilter::default(),
            supply,
            vault,
            options,
            quantity: 1,
            meta: None,
        }
    }

    pub fn uda(vault: DecryptedWalletData, media: Option<IssueMedia>) -> Self {
        let options = IssueContractOptions {
            proxy: true,
            ..Default::default()
        };

        let contract_media = if media.is_some() {
            Some(media.unwrap_or_default())
        } else {
            Some(get_hashed_media())
        };

        Self {
            iface: AssetType::RGB21.to_string(),
            supply: ContractAmount::new(1, 0, 0).to_string(),
            filter: UtxoFilter::default(),
            meta: contract_media,
            quantity: 1,
            vault,
            options,
        }
    }
}

#[derive(Default)]
pub struct WalletSeed {
    pub amount: Option<String>,
    pub mnemonic: Option<String>,
}

impl WalletSeed {
    pub fn with(mnemonic: String, amount: String) -> Self {
        Self {
            mnemonic: Some(mnemonic),
            amount: Some(amount),
        }
    }
}

pub async fn create_new_wallet(seed: WalletSeed) -> Result<DecryptedWalletData, anyhow::Error> {
    let keys = new_mnemonic(&SecretString("".to_string())).await?;

    let iface = "RGB20";
    let watcher_name = "default";

    let PrivateWalletData {
        nostr_prv: ref sk, ..
    } = keys.private.clone();
    let PublicWalletData {
        ref rgb_descriptor_xpub,
        ref btc_descriptor_xpub,
        ..
    } = keys.public.clone();
    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        btc_xpub: btc_descriptor_xpub.to_string(),
        rgb_xpub: rgb_descriptor_xpub.to_string(),
        recover: false,
    };

    let watcher_resp = create_watcher(sk, create_watch_req.clone()).await?;

    let next_address = watcher_next_address(sk, iface, false).await?;
    let amount = seed.amount.map_or("1".to_string(), |x| x.to_string());
    send_coins(&next_address.address, &amount);

    Ok(keys)
}

#[derive(Default)]
pub struct FundSeed {
    pub btc_amount: Option<String>,
    pub sats_fund: Option<String>,
    pub mnemonic: Option<String>,
}

impl FundSeed {
    pub fn new(mnemonic: String, amount: String) -> Self {
        Self {
            mnemonic: Some(mnemonic),
            btc_amount: Some(amount),
            sats_fund: None,
        }
    }
}

pub async fn create_new_fund_wallet(seed: FundSeed) -> Result<DecryptedWalletData, anyhow::Error> {
    let keys = if let Some(seed) = seed.mnemonic {
        save_mnemonic(&SecretString(seed), &SecretString("".to_string())).await?
    } else {
        new_mnemonic(&SecretString("".to_string())).await?
    };

    let btc = "Bitcoin";
    let rgb = "RGB20";
    let watcher_name = "default";

    let PrivateWalletData {
        nostr_prv: ref sk,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        ..
    } = keys.private.clone();

    let PublicWalletData {
        ref rgb_descriptor_xpub,
        ref btc_descriptor_xpub,
        ..
    } = keys.public.clone();

    let create_watch_req = WatcherRequest {
        name: watcher_name.to_string(),
        btc_xpub: btc_descriptor_xpub.to_string(),
        rgb_xpub: rgb_descriptor_xpub.to_string(),
        recover: false,
    };

    let watcher_resp = create_watcher(sk, create_watch_req.clone()).await?;

    let next_address = watcher_next_address(sk, btc, false).await?;
    let amount = seed.btc_amount.map_or("1".to_string(), |x| x.to_string());
    send_coins(&next_address.address, &amount);
    new_block();

    // 0.00001000
    let mut options = FundOptions::with(true, vec![SecretString(btc_descriptor_xprv.to_string())]);

    if let Some(dest) = seed.sats_fund {
        options.fund_value = Some(Sats::from_str(&dest).expect("Invalid dest format"));
    };

    let resp = fund_vault(sk, 1.1, options).await?;
    assert!(resp.rgb_output.is_some());
    Ok(keys)
}

pub async fn create_new_contract(seed: ContractSeed) -> Result<Vec<IssueResponse>, anyhow::Error> {
    let ContractSeed {
        iface: asset_type,
        supply,
        vault,
        options,
        meta,
        quantity: number_of_contracts,
        ..
    } = seed;

    let PrivateWalletData {
        nostr_prv: ref _sk, ..
    } = vault.private.clone();

    let PublicWalletData {
        ref rgb_descriptor_xpub,
        ..
    } = vault.public.clone();

    let issuer_keys = vault.private.nostr_prv.to_string();

    let next_utxo = watcher_next_utxo(&issuer_keys.to_string(), &asset_type.to_string()).await?;

    debug!(format!("issuer Derivation {:?}", rgb_descriptor_xpub));
    debug!(format!("issuer UTXO {:?}", next_utxo));

    let next_utxo = next_utxo.utxo.unwrap_or(String::new());

    info!("Create Contract (Issuer)");
    let mut contracts = vec![];
    for _ in 0..number_of_contracts {
        let issue_seal = next_utxo.clone();
        let issue_supply = ContractAmount::convert(supply.clone());

        let contract_req = IssueRequest {
            ticker: "DIBA".to_string(),
            name: "DIBA".to_string(),
            description: "DIBA".to_string(),
            precision: issue_supply.precision,
            supply: issue_supply.to_string(),
            seal: issue_seal.to_owned(),
            iface: asset_type.to_string(),
            meta: meta.clone(),
            ..Default::default()
        };

        let contract = issue_contract(
            &issuer_keys.to_string(),
            contract_req.clone(),
            options.clone(),
        )
        .await?;
        contracts.push(contract);
    }

    Ok(contracts)
}

pub struct SeedInvoice {
    pub contract_id: Option<String>,
    pub iface: String,
    pub vault: DecryptedWalletData,
    pub amount: RgbInvoiceValue,
    pub chain: ChainTarget,
    pub filter: UtxoFilter,
    pub contract_import: Option<String>,
    pub options: Option<RgbInvoiceOptions>,
}

impl SeedInvoice {
    pub fn new(
        contract_id: ContractId,
        amount: RgbInvoiceValue,
        owner_keys: DecryptedWalletData,
    ) -> Self {
        Self {
            contract_id: Some(contract_id.to_string()),
            iface: String::new(),
            filter: UtxoFilter::default(),
            chain: ChainTarget::Bitcoin,
            vault: owner_keys,
            amount,
            contract_import: None,
            options: None,
        }
    }

    pub fn with_contract(
        contract: String,
        amount: RgbInvoiceValue,
        owner_keys: DecryptedWalletData,
    ) -> Self {
        let contract_bindle = Contract::from_str(&contract).expect("invalid contract (seeder)");
        let contrat_id = contract_bindle.contract_id();

        Self {
            contract_id: Some(contrat_id.to_string()),
            iface: String::new(),
            filter: UtxoFilter::default(),
            chain: ChainTarget::Bitcoin,
            contract_import: Some(format!("{contract_bindle}")),
            vault: owner_keys,
            amount,
            options: None,
        }
    }
}

pub async fn create_new_invoice(seed: SeedInvoice) -> Result<RgbInvoiceResponse, anyhow::Error> {
    let SeedInvoice {
        contract_id,
        iface,
        amount,
        vault,
        contract_import,
        ..
    } = seed;

    let PrivateWalletData {
        nostr_prv: ref sk, ..
    } = vault.private.clone();

    let PublicWalletData {
        ref rgb_descriptor_xpub,
        ..
    } = vault.public.clone();

    debug!(format!("New Owner Derivation {rgb_descriptor_xpub}"));
    if let Some(contract) = contract_import {
        import_contract(&sk.to_string(), contract).await?;
        debug!("Owner Import Contract".to_string());
    }

    let invoice_req = RgbInvoiceRequest {
        seal: RgbSelection::NextUTXO,
        contract_id,
        amount,
        expire_at: none!(),
    };
    let resp = create_invoice(&sk.to_string(), invoice_req, default!()).await?;
    Ok(resp)
}

pub struct SeedTransfer {
    pub src_vault: DecryptedWalletData,
    pub dst_vault: DecryptedWalletData,
    pub transfer_value: RgbInvoiceValue,
    pub asset_type: AssetType,
    pub proxy: bool,
    pub unknown_contract: bool,
    pub fee: Option<ChainFee>,
}

impl SeedTransfer {
    pub fn new(
        asset_type: AssetType,
        transfer_value: RgbInvoiceValue,
        src_vault: DecryptedWalletData,
        dst_vault: DecryptedWalletData,
    ) -> Self {
        Self {
            src_vault,
            dst_vault,
            asset_type,
            transfer_value,
            proxy: false,
            unknown_contract: false,
            fee: None,
        }
    }

    pub fn with_proxy(
        asset_type: AssetType,
        transfer_value: RgbInvoiceValue,
        src_vault: DecryptedWalletData,
        dst_vault: DecryptedWalletData,
    ) -> Self {
        Self {
            src_vault,
            dst_vault,
            asset_type,
            transfer_value,
            proxy: true,
            unknown_contract: false,
            fee: None,
        }
    }

    pub fn with_fee(
        asset_type: AssetType,
        transfer_value: RgbInvoiceValue,
        src_vault: DecryptedWalletData,
        dst_vault: DecryptedWalletData,
        fee: ChainFee,
    ) -> Self {
        Self {
            src_vault,
            dst_vault,
            asset_type,
            transfer_value,
            proxy: false,
            unknown_contract: false,
            fee: Some(fee),
        }
    }
}

pub async fn create_contract_and_transfer(
    seed: SeedTransfer,
) -> Result<(ContractId, RgbTransferResponse), anyhow::Error> {
    let SeedTransfer {
        src_vault,
        dst_vault,
        transfer_value,
        asset_type,
        proxy,
        unknown_contract,
        fee,
        ..
    } = seed;

    let issue_req = match asset_type {
        AssetType::RGB20 => ContractSeed::fungible(src_vault.clone(), none!()),
        AssetType::RGB21 => {
            let media = if proxy {
                Some(get_uri_media())
            } else {
                none!()
            };

            let mut issue_req = ContractSeed::uda(src_vault.clone(), media);
            issue_req.options.proxy = proxy;
            issue_req
        }
        _ => {
            return Err(anyhow::Error::msg(
                "invalid asset type (seeder)".to_string(),
            ));
        }
    };

    let issue_res = create_new_contract(issue_req).await?;
    let issue_res = issue_res[0].clone();
    let ContractResponse {
        contract_id,
        iface,
        contract,
        ..
    } = issue_res;

    info!("Create Invoice".to_string());
    let mut new_invoice = SeedInvoice::with_contract(contract, transfer_value, dst_vault);
    if unknown_contract {
        new_invoice.contract_id = none!();
    }

    let invoice_resp = create_new_invoice(new_invoice).await?;
    assert_ne!(invoice_resp.invoice, String::new());

    let chain_fee = if let Some(chain_fee) = fee {
        chain_fee
    } else {
        ChainFee::Value(1000)
    };

    info!("Create Transfer".to_string());
    let transfer_req = RgbTransferRequest {
        contract_id: contract_id.to_string(),
        iface: iface.to_string(),
        invoice: invoice_resp.invoice.to_string(),
        sender_amount: None,
        chain: ChainTarget::Bitcoin,
        chain_fee,
    };

    let PrivateWalletData {
        nostr_prv: ref sk, ..
    } = src_vault.private;

    let sender_sk = sk;

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ..
    } = src_vault.private.clone();

    let mut options = RgbTransferOptions::broadcast(vec![
        SecretString(rgb_descriptor_xprv.to_string()),
        SecretString(btc_descriptor_xprv.to_string()),
    ]);
    options.proxy = proxy;

    let transfer_resp = transfer_contract(&sender_sk.to_string(), transfer_req, options).await?;
    let contract_id = ContractId::from_str(contract_id.as_str())?;

    Ok((contract_id, transfer_resp))
}

pub struct NewTransfer {
    src_vault: DecryptedWalletData,
    dst_vault: DecryptedWalletData,
    contract_id: ContractId,
    iface: AssetType,
    invoice: RgbInvoice,
    proxy: bool,
    fee: Option<ChainFee>,
}

impl NewTransfer {
    pub fn new(
        src_vault: DecryptedWalletData,
        dst_vault: DecryptedWalletData,
        contract_id: ContractId,
        iface: AssetType,
        invoice: RgbInvoice,
        fee: Option<ChainFee>,
    ) -> Self {
        Self {
            src_vault,
            dst_vault,
            contract_id,
            iface,
            invoice,
            proxy: true,
            fee,
        }
    }
}

pub async fn create_new_transfer(
    seed: NewTransfer,
) -> Result<(ContractId, RgbTransferResponse), anyhow::Error> {
    let NewTransfer {
        src_vault,
        contract_id,
        iface,
        invoice,
        proxy,
        fee,
        ..
    } = seed;

    let chain_fee = if let Some(chain_fee) = fee {
        chain_fee
    } else {
        ChainFee::Value(1000)
    };

    info!("Create Transfer".to_string());
    let transfer_req = RgbTransferRequest {
        contract_id: contract_id.to_string(),
        iface: iface.to_string(),
        invoice: invoice.to_string(),
        sender_amount: None,
        chain: ChainTarget::Bitcoin,
        chain_fee,
    };

    let PrivateWalletData {
        nostr_prv: ref sk, ..
    } = src_vault.private;

    let sender_sk = sk;

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ..
    } = src_vault.private.clone();

    let mut options = RgbTransferOptions::broadcast(vec![
        SecretString(rgb_descriptor_xprv.to_string()),
        SecretString(btc_descriptor_xprv.to_string()),
    ]);
    options.proxy = proxy;

    let transfer_resp = transfer_contract(&sender_sk.to_string(), transfer_req, options).await?;
    Ok((contract_id, transfer_resp))
}
