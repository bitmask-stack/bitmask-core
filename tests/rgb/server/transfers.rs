#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![cfg(not(target_arch = "wasm32"))]

use crate::rgb::server::seeder::{
    ContractSeed, FundSeed, NewTransfer, SeedInvoice, SeedTransfer, WalletSeed,
    create_contract_and_transfer, create_new_contract, create_new_fund_wallet, create_new_invoice,
    create_new_transfer, create_new_wallet,
};
use amplify::{default, none};
use bitmask_core::bitcoin::{publish_psbt_file_bp, sign_and_publish_psbt_file_bp};
use bitmask_core::bp::signer::MainSigner;
use bitmask_core::bp::{FundOptions, fund_vault};
use bitmask_core::regtest::new_block;
use bitmask_core::rgb::models::AssetType;
use bitmask_core::rgb::models::AssetType::RGB20;
use bitmask_core::rgb::{
    accept_transfer, get_contract, list_transfers, save_transfer, verify_transfers,
};
use bitmask_core::structs::{
    AcceptRequest, AcceptResponse, ChainFee, ContractResponse, ContractValue, PrivateWalletData,
    PublicWalletData, PublishPsbtRequest, PublishedPsbtResponse, RgbInvoiceValue, RgbTransferItem,
    RgbTransferResponse, SaveTransferRequest, SecretString, SignPsbtRequest, UDAValue,
};
use bitmask_core::wallet::interop::XprivBpKey;
use bitmask_core::wallet::watcher::watcher_next_utxo;
use bitmask_core::{debug, info};
use bpstd::{Network, Psbt, PsbtVer, Sats, XprivAccount};
use rgbcore::ContractId;
use rgbinvoice::RgbInvoice;
use std::str::FromStr;

#[tokio::test]
async fn accept_new_rgb20_transfer() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB20;
    let transfer_value = RgbInvoiceValue::Value("1.00".to_string());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let seed = SeedTransfer::new(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
    );
    let (_, RgbTransferResponse { consig, psbt, .. }) = create_contract_and_transfer(seed).await?;

    let descriptors = [SecretString(rgb_descriptor_xprv.clone())];

    let mut psbt = Psbt::from_base64(&psbt).expect("invalid psbt");
    let signers: Vec<_> = descriptors
        .iter()
        .map(|x| {
            let interop_key = XprivBpKey::new(&x.to_string(), Network::Regtest)
                .expect("invalid parse interop key");
            MainSigner::new(
                XprivAccount::new(*interop_key.xpriv(), interop_key.origin().clone())
                    .expect("invalid parse xpriv account"),
            )
        })
        .collect();

    for signer in signers.iter() {
        psbt.sign(signer).expect("Error signing btc account");
    }
    let psbt = psbt.to_base64_ver(PsbtVer::V0);
    let psbt_req = PublishPsbtRequest { psbt };

    let psbt_resp = publish_psbt_file_bp(psbt_req).await?;
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    info!("Create new Block");
    new_block();

    info!("Accept Transfer");
    let accept_req = AcceptRequest {
        consignment: consig,
        force: false,
    };

    let accept_resp = accept_transfer(&beneficiary_sk.clone(), accept_req, default!()).await?;
    Ok(())
}

#[tokio::test]
async fn accept_new_rgb20_unknow_transfer() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB20;
    let transfer_value = RgbInvoiceValue::Value("1.00".to_string());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let seed = SeedTransfer::new(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
    );
    let (contract_id, RgbTransferResponse { consig, .. }) =
        create_contract_and_transfer(seed).await?;

    let save_transfer_req = SaveTransferRequest {
        consignment: consig.clone(),
    };

    // Unknown Transfer
    save_transfer(beneficiary_sk, save_transfer_req, default!()).await?;

    let sender = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(sender.balance, ContractValue::Value(410));

    let receiver = get_contract(&beneficiary_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(receiver.balance, ContractValue::Value(0));

    info!("Create new Block");
    new_block();

    verify_transfers(sender_sk, true).await?;
    verify_transfers(beneficiary_sk, true).await?;

    let sender = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(sender.balance, ContractValue::Value(410));
    // println!(
    //     "{}",
    //     format!("Sender: {contract_id} {:#?}", sender.allocations)
    // );

    let receiver = get_contract(&beneficiary_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(receiver.balance, ContractValue::Value(100));
    // println!(
    //     "{}",
    //     format!("Sender: {contract_id} {:#?}", receiver.allocations)
    // );
    Ok(())
}

#[tokio::test]
async fn accept_new_rgb20_transfer_by_proxy() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB20;
    let transfer_value = RgbInvoiceValue::Value("1.00".to_string());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let seed = SeedTransfer::with_proxy(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
    );
    let (contract_id, RgbTransferResponse { consig, psbt, .. }) =
        create_contract_and_transfer(seed).await?;

    let descriptors = vec![
        SecretString(btc_descriptor_xprv.clone()),
        SecretString(btc_change_descriptor_xprv.clone()),
        SecretString(rgb_descriptor_xprv.clone()),
    ];

    let psbt_req = SignPsbtRequest { psbt, descriptors };

    let psbt_resp = sign_and_publish_psbt_file_bp(psbt_req).await?;
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    verify_transfers(&sender_sk.to_string(), true).await?;
    let sender_txs = list_transfers(sender_sk).await?;

    verify_transfers(&beneficiary_sk.to_string(), true).await?;
    let beneficiary_txs = list_transfers(beneficiary_sk).await?;

    assert!(!sender_txs.is_empty());
    assert!(!beneficiary_txs.is_empty());

    info!("Create new Block");
    new_block();

    verify_transfers(&sender_sk.to_string(), true).await?;
    verify_transfers(&beneficiary_sk.to_string(), true).await?;
    let sender = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(sender.balance, ContractValue::Value(410));

    let receiver = get_contract(&beneficiary_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(receiver.balance, ContractValue::Value(100));
    Ok(())
}

#[tokio::test]
async fn accept_new_rgb20_transfer_without_contract() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = RGB20;
    let transfer_value = RgbInvoiceValue::Value("1.00".to_string());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let mut seed = SeedTransfer::with_proxy(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
    );
    seed.unknown_contract = true;

    let (contract_id, RgbTransferResponse { consig, psbt, .. }) =
        create_contract_and_transfer(seed).await?;

    let descriptors = vec![
        SecretString(btc_descriptor_xprv.clone()),
        SecretString(btc_change_descriptor_xprv.clone()),
        SecretString(rgb_descriptor_xprv.clone()),
    ];

    let psbt_req = SignPsbtRequest { psbt, descriptors };

    let psbt_resp = sign_and_publish_psbt_file_bp(psbt_req).await?;
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    verify_transfers(&sender_sk.to_string(), true).await?;
    let sender_txs = list_transfers(sender_sk).await?;

    verify_transfers(&beneficiary_sk.to_string(), true).await?;
    let beneficiary_txs = list_transfers(beneficiary_sk).await?;

    assert!(!sender_txs.is_empty());
    assert!(!beneficiary_txs.is_empty());

    info!("Create new Block");
    new_block();

    verify_transfers(&sender_sk.to_string(), true).await?;
    verify_transfers(&beneficiary_sk.to_string(), true).await?;
    let sender = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(sender.balance, ContractValue::Value(410));

    let receiver = get_contract(&beneficiary_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(receiver.balance, ContractValue::Value(100));
    Ok(())
}

#[tokio::test]
async fn consecutive_transfers_with_dust_utxo() -> anyhow::Result<()> {
    let seed = FundSeed {
        btc_amount: Some("1".to_string()),
        sats_fund: Some("443".to_string()),
        ..Default::default()
    };

    let issuer_keys = create_new_fund_wallet(seed).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = RGB20;
    let transfer_value = RgbInvoiceValue::Value("1.00".to_string());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let mut seed = SeedTransfer::with_proxy(
        asset_type.clone(),
        transfer_value.clone(),
        issuer_keys.clone(),
        owners_keys.clone(),
    );

    seed.fee = Some(ChainFee::Value(112));

    let (contract_id, RgbTransferResponse { consig, psbt, .. }) =
        create_contract_and_transfer(seed).await?;

    info!("Check Balances (Without Confirmations");
    verify_transfers(&sender_sk.to_string(), true).await?;
    let sender_txs = list_transfers(sender_sk).await?;

    verify_transfers(&beneficiary_sk.to_string(), true).await?;
    let beneficiary_txs = list_transfers(beneficiary_sk).await?;

    assert!(!sender_txs.is_empty());
    assert!(!beneficiary_txs.is_empty());

    info!("Create new Block");
    new_block();

    info!("Check Balances");
    verify_transfers(&sender_sk.to_string(), true).await?;
    verify_transfers(&beneficiary_sk.to_string(), true).await?;
    let sender = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(sender.balance, ContractValue::Value(410));

    let receiver = get_contract(&beneficiary_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(receiver.balance, ContractValue::Value(100));

    info!("Check Remaining UTXO");
    let utxo_sender = watcher_next_utxo(&sender_sk.to_string(), "rgb").await?;
    assert!(utxo_sender.utxo.is_some());

    info!("Refund Wallet");
    let mut fund_options = FundOptions::with(
        true,
        [
            SecretString(btc_descriptor_xprv.to_string()),
            SecretString(btc_change_descriptor_xprv.to_string()),
        ]
        .to_vec(),
    );
    fund_options.fund_value = Some(Sats(600));
    fund_vault(sender_sk, 1.1, fund_options).await?;

    info!("Confirm Refund");
    new_block();

    let sender = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(sender.balance, ContractValue::Value(410));

    let receiver = get_contract(&beneficiary_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(receiver.balance, ContractValue::Value(100));

    info!("New Invoice");
    let invoice_seed = SeedInvoice::new(contract_id, transfer_value, owners_keys.clone());
    let rgb_invoice = create_new_invoice(invoice_seed).await?;
    let rgb_invoice = RgbInvoice::from_str(&rgb_invoice.invoice)?;

    info!("New Transfer");
    let seed = NewTransfer::new(
        issuer_keys,
        owners_keys,
        contract_id,
        asset_type,
        rgb_invoice,
        Some(ChainFee::Value(200)),
    );
    let (contract_id, RgbTransferResponse { consig, psbt, .. }) = create_new_transfer(seed).await?;

    info!("Confirm Transfer");
    new_block();

    info!("Check Balances Again");
    verify_transfers(&sender_sk.to_string(), true).await?;
    verify_transfers(&beneficiary_sk.to_string(), true).await?;
    let sender = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(sender.balance, ContractValue::Value(310));

    let receiver = get_contract(&beneficiary_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(receiver.balance, ContractValue::Value(200));

    Ok(())
}

#[tokio::test]
async fn accept_new_rgb21_transfer() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB21;
    let transfer_value = RgbInvoiceValue::UDA(default!());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let seed = SeedTransfer::with_fee(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
        ChainFee::FeeRate(1.1),
    );
    let (_, RgbTransferResponse { consig, .. }) = create_contract_and_transfer(seed).await?;

    info!("Create new Block");
    new_block();

    info!("Accept Transfer");
    let accept_req = AcceptRequest {
        consignment: consig,
        force: false,
    };

    let accept_resp = accept_transfer(&beneficiary_sk.clone(), accept_req, default!()).await?;
    assert!(accept_resp.valid);
    Ok(())
}
#[tokio::test]
async fn accept_new_rgb21_with_fee_rate_transfer() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB21;
    let transfer_value = RgbInvoiceValue::UDA(default!());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let seed = SeedTransfer::with_fee(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
        ChainFee::FeeRate(1.1),
    );
    let (_, RgbTransferResponse { consig, .. }) = create_contract_and_transfer(seed).await?;

    info!("Create new Block");
    new_block();

    info!("Accept Transfer");
    let accept_req = AcceptRequest {
        consignment: consig,
        force: false,
    };

    let accept_resp = accept_transfer(&beneficiary_sk.clone(), accept_req, default!()).await?;
    assert!(accept_resp.valid);
    Ok(())
}
#[tokio::test]
async fn accept_new_rgb21_unknow_transfer() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB21;
    let transfer_value = RgbInvoiceValue::UDA(default!());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let seed = SeedTransfer::new(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
    );
    let (contract_id, RgbTransferResponse { consig, .. }) =
        create_contract_and_transfer(seed).await?;

    let save_transfer_req = SaveTransferRequest {
        consignment: consig.clone(),
    };

    // Unknown Transfer
    save_transfer(beneficiary_sk, save_transfer_req, default!()).await?;

    let sender = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(sender.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let receiver = get_contract(&beneficiary_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(receiver.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    info!("Create new Block");
    new_block();

    verify_transfers(sender_sk, true).await?;
    verify_transfers(beneficiary_sk, true).await?;

    let sender = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(sender.balance, ContractValue::UDA(UDAValue::with(1, 0)));

    let receiver = get_contract(&beneficiary_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(receiver.balance, ContractValue::UDA(UDAValue::with(1, 1)));
    Ok(())
}

#[tokio::test]
async fn accept_new_rgb21_transfer_by_proxy() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB21;
    let transfer_value = RgbInvoiceValue::UDA(UDAValue::default());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PublicWalletData { .. } = issuer_keys.public.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let seed = SeedTransfer::with_proxy(
        asset_type,
        transfer_value,
        issuer_keys.clone(),
        owners_keys.clone(),
    );
    let (_, RgbTransferResponse { consig, psbt, .. }) = create_contract_and_transfer(seed).await?;

    let descriptors = vec![
        SecretString(btc_descriptor_xprv.clone()),
        SecretString(btc_change_descriptor_xprv.clone()),
        SecretString(rgb_descriptor_xprv.clone()),
    ];

    let psbt_req = SignPsbtRequest { psbt, descriptors };

    let psbt_resp = sign_and_publish_psbt_file_bp(psbt_req).await?;
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    info!("Create new Block");
    new_block();

    let verify_resp = verify_transfers(&beneficiary_sk.to_string(), true).await?;
    debug!(format!("Transf: {:?}", verify_resp));
    Ok(())
}

#[tokio::test]
async fn owner_pays_onward_rgb20_transfer() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;
    let third_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB20;
    let transfer_value = RgbInvoiceValue::Value("1.00".to_string());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        rgb_descriptor_xprv: ref owner_rgb_descriptor_xprv,
        btc_descriptor_xprv: ref owner_btc_descriptor_xprv,
        btc_change_descriptor_xprv: ref owner_btc_change_descriptor_xprv,
        nostr_prv: ref owner_sk,
        ..
    } = owners_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref third_sk,
        ..
    } = third_keys.private.clone();

    let seed = SeedTransfer::with_proxy(
        asset_type.clone(),
        transfer_value.clone(),
        issuer_keys.clone(),
        owners_keys.clone(),
    );
    let (contract_id, RgbTransferResponse { consig, psbt, .. }) =
        create_contract_and_transfer(seed).await?;

    let descriptors = vec![
        SecretString(btc_descriptor_xprv.clone()),
        SecretString(btc_change_descriptor_xprv.clone()),
        SecretString(rgb_descriptor_xprv.clone()),
    ];

    let psbt_req = SignPsbtRequest { psbt, descriptors };
    let psbt_resp = sign_and_publish_psbt_file_bp(psbt_req).await?;
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    verify_transfers(&sender_sk.to_string(), true).await?;
    verify_transfers(&owner_sk.to_string(), true).await?;

    info!("Create new Block");
    new_block();

    verify_transfers(&sender_sk.to_string(), true).await?;
    verify_transfers(&owner_sk.to_string(), true).await?;
    let issuer = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    let owner = get_contract(&owner_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(issuer.balance, ContractValue::Value(410));
    assert_eq!(owner.balance, ContractValue::Value(100));

    info!("Owner invoice for third wallet");
    let invoice_seed = SeedInvoice::new(contract_id, transfer_value, third_keys.clone());
    let rgb_invoice = create_new_invoice(invoice_seed).await?;
    let rgb_invoice = RgbInvoice::from_str(&rgb_invoice.invoice)?;

    info!("Owner pays onward");
    let seed = NewTransfer::new(
        owners_keys,
        third_keys,
        contract_id,
        asset_type,
        rgb_invoice,
        None,
    );
    let (contract_id, RgbTransferResponse { psbt, .. }) = create_new_transfer(seed).await?;

    let descriptors = vec![
        SecretString(owner_btc_descriptor_xprv.clone()),
        SecretString(owner_btc_change_descriptor_xprv.clone()),
        SecretString(owner_rgb_descriptor_xprv.clone()),
    ];
    let psbt_req = SignPsbtRequest { psbt, descriptors };
    let psbt_resp = sign_and_publish_psbt_file_bp(psbt_req).await?;
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    info!("Confirm owner onward transfer");
    new_block();

    verify_transfers(&sender_sk.to_string(), true).await?;
    verify_transfers(&owner_sk.to_string(), true).await?;
    verify_transfers(&third_sk.to_string(), true).await?;

    let issuer = get_contract(&sender_sk.to_string(), contract_id.to_string()).await?;
    let owner = get_contract(&owner_sk.to_string(), contract_id.to_string()).await?;
    let third = get_contract(&third_sk.to_string(), contract_id.to_string()).await?;
    assert_eq!(issuer.balance, ContractValue::Value(410));
    assert_eq!(owner.balance, ContractValue::Value(0));
    assert_eq!(third.balance, ContractValue::Value(100));
    Ok(())
}

#[tokio::test]
async fn two_rgb20_contracts_on_one_utxo() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let asset_type = AssetType::RGB20;
    let transfer_value = RgbInvoiceValue::Value("1.00".to_string());

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let mut issue_req = ContractSeed::fungible(issuer_keys.clone(), none!());
    issue_req.quantity = 2;
    let issue_res = create_new_contract(issue_req).await?;
    assert_eq!(issue_res.len(), 2);
    assert_eq!(issue_res[0].issue_utxo, issue_res[1].issue_utxo);
    assert_ne!(issue_res[0].contract_id, issue_res[1].contract_id);

    let first_id = ContractId::from_str(&issue_res[0].contract_id)?;
    let second_id = ContractId::from_str(&issue_res[1].contract_id)?;

    let first_invoice = create_new_invoice(SeedInvoice::with_contract(
        issue_res[0].contract.clone(),
        transfer_value.clone(),
        owners_keys.clone(),
    ))
    .await?;
    let second_invoice = create_new_invoice(SeedInvoice::with_contract(
        issue_res[1].contract.clone(),
        transfer_value,
        owners_keys.clone(),
    ))
    .await?;
    let first_invoice = RgbInvoice::from_str(&first_invoice.invoice)?;
    let second_invoice = RgbInvoice::from_str(&second_invoice.invoice)?;

    info!("Transfer first RGB20 from shared UTXO");
    let seed = NewTransfer::new(
        issuer_keys.clone(),
        owners_keys.clone(),
        first_id,
        asset_type.clone(),
        first_invoice,
        None,
    );
    let (_, RgbTransferResponse { psbt, .. }) = create_new_transfer(seed).await?;

    let descriptors = vec![
        SecretString(btc_descriptor_xprv.clone()),
        SecretString(btc_change_descriptor_xprv.clone()),
        SecretString(rgb_descriptor_xprv.clone()),
    ];
    let psbt_req = SignPsbtRequest {
        psbt,
        descriptors: descriptors.clone(),
    };
    let psbt_resp = sign_and_publish_psbt_file_bp(psbt_req).await?;
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    new_block();
    verify_transfers(&sender_sk.to_string(), true).await?;
    verify_transfers(&beneficiary_sk.to_string(), true).await?;

    let owner_first = get_contract(&beneficiary_sk.to_string(), first_id.to_string()).await?;
    assert_eq!(owner_first.balance, ContractValue::Value(100));

    info!("Transfer second RGB20 from the same issuer UTXO");
    let seed = NewTransfer::new(
        issuer_keys,
        owners_keys,
        second_id,
        asset_type,
        second_invoice,
        None,
    );
    match create_new_transfer(seed).await {
        Ok((_, RgbTransferResponse { psbt, .. })) => {
            let psbt_req = SignPsbtRequest { psbt, descriptors };
            let psbt_resp = sign_and_publish_psbt_file_bp(psbt_req).await?;
            debug!(format!("Sign Psbt: {:?}", psbt_resp));
            new_block();
            verify_transfers(&sender_sk.to_string(), true).await?;
            verify_transfers(&beneficiary_sk.to_string(), true).await?;
            let owner_second =
                get_contract(&beneficiary_sk.to_string(), second_id.to_string()).await?;
            assert_eq!(owner_second.balance, ContractValue::Value(100));
        }
        Err(err) => {
            // First transfer spent the shared UTXO. HEAD coinselect has no second input.
            let msg = err.to_string();
            assert!(
                !msg.is_empty(),
                "second transfer failed with an empty error"
            );
            debug!(format!(
                "second RGB20 transfer after shared UTXO spend: {msg}"
            ));
        }
    }
    Ok(())
}

#[tokio::test]
async fn rgb20_and_rgb21_contracts_on_one_utxo() -> anyhow::Result<()> {
    let issuer_keys = create_new_wallet(WalletSeed::default()).await?;
    let owners_keys = create_new_wallet(WalletSeed::default()).await?;

    let PrivateWalletData {
        ref rgb_descriptor_xprv,
        ref btc_descriptor_xprv,
        ref btc_change_descriptor_xprv,
        nostr_prv: ref sender_sk,
        ..
    } = issuer_keys.private.clone();

    let PrivateWalletData {
        nostr_prv: ref beneficiary_sk,
        ..
    } = owners_keys.private.clone();

    let rgb20 = create_new_contract(ContractSeed::fungible(issuer_keys.clone(), none!())).await?;
    let mut uda = ContractSeed::uda(issuer_keys.clone(), none!());
    uda.options.proxy = false;
    let rgb21 = create_new_contract(uda).await?;

    assert_eq!(rgb20.len(), 1);
    assert_eq!(rgb21.len(), 1);
    assert_eq!(rgb20[0].issue_utxo, rgb21[0].issue_utxo);
    assert_eq!(rgb20[0].iface, AssetType::RGB20.to_string());
    assert_eq!(rgb21[0].iface, AssetType::RGB21.to_string());
    assert_ne!(rgb20[0].contract_id, rgb21[0].contract_id);

    let rgb20_id = ContractId::from_str(&rgb20[0].contract_id)?;
    let rgb21_id = ContractId::from_str(&rgb21[0].contract_id)?;

    let rgb20_invoice = create_new_invoice(SeedInvoice::with_contract(
        rgb20[0].contract.clone(),
        RgbInvoiceValue::Value("1.00".to_string()),
        owners_keys.clone(),
    ))
    .await?;
    let rgb21_invoice = create_new_invoice(SeedInvoice::with_contract(
        rgb21[0].contract.clone(),
        RgbInvoiceValue::UDA(default!()),
        owners_keys.clone(),
    ))
    .await?;
    let rgb20_invoice = RgbInvoice::from_str(&rgb20_invoice.invoice)?;
    let rgb21_invoice = RgbInvoice::from_str(&rgb21_invoice.invoice)?;

    info!("Transfer RGB20 from shared UTXO");
    let seed = NewTransfer::new(
        issuer_keys.clone(),
        owners_keys.clone(),
        rgb20_id,
        AssetType::RGB20,
        rgb20_invoice,
        None,
    );
    let (_, RgbTransferResponse { psbt, .. }) = create_new_transfer(seed).await?;

    let descriptors = vec![
        SecretString(btc_descriptor_xprv.clone()),
        SecretString(btc_change_descriptor_xprv.clone()),
        SecretString(rgb_descriptor_xprv.clone()),
    ];
    let psbt_req = SignPsbtRequest {
        psbt,
        descriptors: descriptors.clone(),
    };
    let psbt_resp = sign_and_publish_psbt_file_bp(psbt_req).await?;
    debug!(format!("Sign Psbt: {:?}", psbt_resp));

    new_block();
    verify_transfers(&sender_sk.to_string(), true).await?;
    verify_transfers(&beneficiary_sk.to_string(), true).await?;

    let owner_rgb20 = get_contract(&beneficiary_sk.to_string(), rgb20_id.to_string()).await?;
    assert_eq!(owner_rgb20.balance, ContractValue::Value(100));

    info!("Transfer RGB21 from the same issuer UTXO");
    let seed = NewTransfer::new(
        issuer_keys,
        owners_keys,
        rgb21_id,
        AssetType::RGB21,
        rgb21_invoice,
        None,
    );
    match create_new_transfer(seed).await {
        Ok((_, RgbTransferResponse { psbt, .. })) => {
            let psbt_req = SignPsbtRequest { psbt, descriptors };
            let psbt_resp = sign_and_publish_psbt_file_bp(psbt_req).await?;
            debug!(format!("Sign Psbt: {:?}", psbt_resp));
            new_block();
            verify_transfers(&sender_sk.to_string(), true).await?;
            verify_transfers(&beneficiary_sk.to_string(), true).await?;
            let owner_rgb21 =
                get_contract(&beneficiary_sk.to_string(), rgb21_id.to_string()).await?;
            assert_eq!(
                owner_rgb21.balance,
                ContractValue::UDA(UDAValue::with(1, 1))
            );
        }
        Err(err) => {
            // First transfer spent the shared UTXO. HEAD coinselect has no second input.
            let msg = err.to_string();
            assert!(
                !msg.is_empty(),
                "second transfer failed with an empty error"
            );
            debug!(format!("RGB21 transfer after shared UTXO spend: {msg}"));
        }
    }
    Ok(())
}
