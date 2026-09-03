#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(target_arch = "wasm32")]

use crate::rgb::web::utils::send_coins;
use amplify::none;
use bitmask_core::rgb::issue::IssueContractOptions;
use bitmask_core::rgb::models::{AssetType, ContractAmount};
use bitmask_core::rgb::transfer::RgbInvoiceOptions;
use bitmask_core::structs::{
    ChainFee, ChainTarget, ContractResponse, DecryptedWalletData, FundVaultDetails, IssueMedia,
    IssueRequest, IssueResponse, MediaData, NextAddressResponse, NextUtxoResponse,
    PrivateWalletData, PublicWalletData, RgbInvoiceRequest, RgbInvoiceResponse, RgbInvoiceValue,
    RgbSelection, RgbTransferRequest, RgbTransferResponse, SecretString, WatcherRequest,
    WatcherResponse,
};
use bitmask_core::web::bitcoin::{fund_vault, get_wallet_data, new_mnemonic};
use bitmask_core::web::bp::{fund_rgb_vault, get_btc_wallet};
use bitmask_core::web::rgb::{
    create_and_publish_rgb_transfer, create_rgb_invoice, create_rgb_transfer, import_contract,
    issue_contract, issue_contract_proxy,
};
use bitmask_core::web::wallet::{create_watcher, watcher_next_address, watcher_next_utxo};
use bitmask_core::web::{json_parse, resolve};
use bitmask_core::{debug, info};
use rgbstd::containers::{ConsignmentExt, Contract};
use std::str::FromStr;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test_configure;

wasm_bindgen_test_configure!(run_in_browser);

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

pub async fn create_new_wallet(seed: WalletSeed) -> Result<DecryptedWalletData, anyhow::Error> {
    let keys = resolve(new_mnemonic("".to_string())).await;
    let keys: DecryptedWalletData = json_parse(&keys);

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

    let create_watch_req =
        serde_wasm_bindgen::to_value(&create_watch_req).expect("failed to serialize");

    let watcher_resp: JsValue =
        resolve(create_watcher(sk.to_string(), create_watch_req.clone())).await;
    let _: WatcherResponse = json_parse(&watcher_resp);

    let next_address: JsValue =
        resolve(watcher_next_address(sk.to_string(), iface.to_string())).await;
    let next_address: NextAddressResponse = json_parse(&next_address);

    let amount = seed.amount.map_or("1".to_string(), |x| x.to_string());
    let _ = send_coins(&next_address.address, &amount).await;

    Ok(keys)
}

pub async fn create_new_fund_wallet(
    seed: WalletSeed,
) -> Result<DecryptedWalletData, anyhow::Error> {
    let keys = resolve(new_mnemonic("".to_string())).await;
    let keys: DecryptedWalletData = json_parse(&keys);

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

    let create_watch_req =
        serde_wasm_bindgen::to_value(&create_watch_req).expect("failed to serialize");

    let watcher_resp: JsValue =
        resolve(create_watcher(sk.to_string(), create_watch_req.clone())).await;
    let _: WatcherResponse = json_parse(&watcher_resp);

    let btc_next_address: JsValue =
        resolve(watcher_next_address(sk.to_string(), btc.to_string())).await;
    let next_address: NextAddressResponse = json_parse(&btc_next_address);

    let amount = seed.amount.map_or("1".to_string(), |x| x.to_string());
    let _ = send_coins(&next_address.address, &amount).await;

    let rgb_next_address: JsValue =
        resolve(watcher_next_address(sk.to_string(), rgb.to_string())).await;
    let rgb_next_address: NextAddressResponse = json_parse(&rgb_next_address);

    let secrets = [
        SecretString(btc_descriptor_xprv.to_string()),
        SecretString(btc_change_descriptor_xprv.to_string()),
    ];
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("invalid request");

    let resp = resolve(fund_rgb_vault(
        sk.clone(),
        1.0,
        secrets,
        true,
        none!(),
        false,
    ))
    .await;
    let _: FundVaultDetails = json_parse(&resp);

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

    let next_utxo: JsValue = resolve(watcher_next_utxo(
        issuer_keys.to_string(),
        asset_type.to_string(),
    ))
    .await;
    debug!(format!("issuer Derivation {:?}", rgb_descriptor_xpub));
    debug!(format!("issuer UTXO {:?}", next_utxo));

    let next_utxo: NextUtxoResponse = json_parse(&next_utxo);
    let next_utxo = next_utxo.utxo.map_or(String::new(), |x| x);

    info!("Create Contract (Issuer)");
    let mut contracts = vec![];
    for _ in 0..number_of_contracts {
        let issue_seal = next_utxo.clone().to_string();
        let issue_supply = ContractAmount::convert(supply.clone());

        let request = IssueRequest {
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

        let contract_req = serde_wasm_bindgen::to_value(&request).expect("invalid request");
        let contract_resp = if options.proxy {
            resolve(issue_contract_proxy(
                issuer_keys.to_string(),
                contract_req.clone(),
            ))
            .await
        } else {
            resolve(issue_contract(
                issuer_keys.to_string(),
                contract_req.clone(),
            ))
            .await
        };

        let contract: IssueResponse = json_parse(&contract_resp);

        contracts.push(contract);
    }

    Ok(contracts)
}

pub async fn create_new_contract_proxy(
    seed: ContractSeed,
) -> Result<Vec<IssueResponse>, anyhow::Error> {
    let ContractSeed {
        iface: asset_type,
        supply,
        vault,
        options,
        meta,
        quantity: number_of_contracts,
        ..
    } = seed;

    let issuer_keys = vault.private.nostr_prv.to_string();

    let next_utxo: JsValue = resolve(watcher_next_utxo(
        issuer_keys.to_string(),
        asset_type.to_string(),
    ))
    .await;
    debug!(format!("issuer UTXO {:?}", next_utxo));

    let next_utxo: NextUtxoResponse = json_parse(&next_utxo);
    let next_utxo = next_utxo.utxo.map_or(String::new(), |x| x);

    info!("Create Contract (Issuer)");
    let mut contracts = vec![];
    for _ in 0..number_of_contracts {
        let issue_seal = next_utxo.clone().to_string();
        let issue_supply = ContractAmount::convert(supply.clone());

        let request = IssueRequest {
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

        let contract_req = serde_wasm_bindgen::to_value(&request).expect("invalid request");
        let contract_resp = resolve(issue_contract_proxy(
            issuer_keys.to_string(),
            contract_req.clone(),
        ))
        .await;
        let contract: IssueResponse = json_parse(&contract_resp);

        contracts.push(contract);
    }

    Ok(contracts)
}

pub struct SeedInvoice {
    pub contract_id: String,
    pub iface: String,
    pub vault: DecryptedWalletData,
    pub amount: RgbInvoiceValue,
    pub chain: ChainTarget,
    pub filter: UtxoFilter,
    pub contract_import: Option<String>,
    pub options: Option<RgbInvoiceOptions>,
}

impl SeedInvoice {
    pub fn with_contract(
        contract: String,
        amount: RgbInvoiceValue,
        owner_keys: DecryptedWalletData,
    ) -> Self {
        let contract_bindle = Contract::from_str(&contract).expect("invalid contract (seeder)");
        let contrat_id = contract_bindle.contract_id();

        Self {
            contract_id: contrat_id.to_string(),
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
        resolve(import_contract(sk.to_string(), contract)).await;
        debug!(format!("Owner Import Contract"));
    }

    let invoice_req = RgbInvoiceRequest {
        seal: RgbSelection::NextUTXO,
        contract_id: Some(contract_id),
        amount,
        expire_at: none!(),
    };
    let invoice_req: JsValue = serde_wasm_bindgen::to_value(&invoice_req).expect("invalid request");
    let resp = json_parse(&resolve(create_rgb_invoice(sk.to_string(), invoice_req)).await);
    Ok(resp)
}

pub struct SeedTransfer {
    pub src_vault: DecryptedWalletData,
    pub dst_vault: DecryptedWalletData,
    pub transfer_value: RgbInvoiceValue,
    pub asset_type: AssetType,
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
        }
    }
}

pub async fn create_new_transfer(seed: SeedTransfer) -> Result<RgbTransferResponse, anyhow::Error> {
    let SeedTransfer {
        src_vault,
        dst_vault,
        transfer_value,
        asset_type,
        ..
    } = seed;

    let issue_req = match asset_type {
        AssetType::RGB20 => ContractSeed::fungible(src_vault.clone(), none!()),
        AssetType::RGB21 => ContractSeed::uda(src_vault.clone(), none!()),
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

    info!(format!("Create Invoice"));
    let new_invoice = SeedInvoice::with_contract(contract, transfer_value, dst_vault);
    let invoice_resp = create_new_invoice(new_invoice).await?;
    assert_ne!(invoice_resp.invoice, String::new());

    info!(format!("Create Transfer"));
    let transfer_req = RgbTransferRequest {
        contract_id: contract_id.to_string(),
        iface: iface.to_string(),
        invoice: invoice_resp.invoice.to_string(),
        sender_amount: None,
        chain: ChainTarget::Bitcoin,
        chain_fee: ChainFee::Value(1000),
    };

    let PrivateWalletData {
        nostr_prv: ref sk, ..
    } = src_vault.private;

    let sender_sk = sk;
    let transfer_req = serde_wasm_bindgen::to_value(&transfer_req).expect("");
    let transfer_resp: RgbTransferResponse =
        json_parse(&resolve(create_rgb_transfer(sender_sk.to_string(), transfer_req)).await);

    Ok(transfer_resp)
}

pub async fn create_and_publish_transfer(
    seed: SeedTransfer,
) -> Result<RgbTransferResponse, anyhow::Error> {
    let SeedTransfer {
        src_vault,
        dst_vault,
        transfer_value,
        asset_type,
        ..
    } = seed;

    let issue_req = match asset_type {
        AssetType::RGB20 => ContractSeed::fungible(src_vault.clone(), none!()),
        AssetType::RGB21 => ContractSeed::uda(src_vault.clone(), none!()),
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

    info!(format!("Create Invoice"));
    let new_invoice = SeedInvoice::with_contract(contract, transfer_value, dst_vault);
    let invoice_resp = create_new_invoice(new_invoice).await?;
    assert_ne!(invoice_resp.invoice, String::new());

    info!(format!("Create Transfer"));
    let transfer_req = RgbTransferRequest {
        contract_id: contract_id.to_string(),
        iface: iface.to_string(),
        invoice: invoice_resp.invoice.to_string(),
        sender_amount: None,
        chain: ChainTarget::Bitcoin,
        chain_fee: ChainFee::Value(1000),
    };

    let PrivateWalletData {
        nostr_prv: ref sk,
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        ..
    } = src_vault.private;

    let sender_sk = sk;

    let secrets = vec![
        SecretString(btc_descriptor_xprv.clone()),
        SecretString(btc_change_descriptor_xprv.clone()),
        SecretString(rgb_descriptor_xprv.clone()),
    ];
    let secrets = serde_wasm_bindgen::to_value(&secrets).expect("");

    let transfer_req = serde_wasm_bindgen::to_value(&transfer_req).expect("");
    let transfer_resp: RgbTransferResponse = json_parse(
        &resolve(create_and_publish_rgb_transfer(
            sender_sk.to_string(),
            transfer_req,
            secrets,
        ))
        .await,
    );

    Ok(transfer_resp)
}
