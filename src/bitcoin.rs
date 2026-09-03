use std::str::FromStr;

use amplify::hex::ToHex;
use argon2::Argon2;
use bdk_wallet::bitcoin::{FeeRate, Network, Txid, address::NetworkUnchecked, psbt::Psbt};
use bdk_wallet::chain::{ChainPosition, ConfirmationBlockTime};
use bdk_wallet::{LocalOutput, SignOptions};
use bitcoin::Address;
use bitcoin::consensus::serialize;
use bpstd::Tx;
use rand::{Rng, SeedableRng, rngs::StdRng};
use thiserror::Error;
use zeroize::Zeroize;

mod assets;
mod keys;
mod payment;
mod psbt;
mod wallet;
mod wallet_encrypt;

use crate::bitcoin::wallet_encrypt::{
    EncryptedMessage, WalletEncryptError, decrypt_owned, encrypt_owned,
};

pub use crate::bitcoin::{
    assets::dust_tx,
    keys::{BitcoinKeysError, get_mnemonic, new_mnemonic, save_mnemonic},
    payment::{BitcoinPaymentError, create_payjoin, create_transaction},
    psbt::{
        BitcoinPsbtError, multi_sign_and_publish_psbt, multi_sign_psbt, publish_psbt,
        sign_and_publish_psbt, sign_psbt,
    },
    wallet::{
        BitcoinWalletError, MemoryWallet, get_blockchain, get_wallet, sync_wallet, sync_wallets,
    },
};

use crate::constants::BITCOIN_EXPLORER_API;
use crate::{
    constants::{DIBA_DESCRIPTOR, DIBA_DESCRIPTOR_VERSION, DIBA_MAGIC_NO, FEE_ADDRESS, NETWORK},
    debug, info,
    structs::{
        Balance, BlockTime, DecryptedWalletData, EncryptedWalletDataV04, FundVaultDetails,
        PublishPsbtRequest, PublishedPsbtResponse, SatsInvoice, SecretString, SignPsbtRequest,
        SignedPsbtResponse, TransactionData, TransactionDetails, WalletData, WalletTransaction,
    },
    trace,
};

// Minimum amount of satoshis to funding vault
pub const MIN_FUNDS_SATS: u64 = 10000;

#[allow(clippy::result_large_err)] // product Bitcoin Result graph; do not reshape
#[derive(Error, Debug)]
pub enum BitcoinError {
    /// Wrong Encrypted Descriptor Format
    #[error(
        "Wrong Format: Encrypted descriptor is not prefixed with DIBA magic number. Prefix was: {0}"
    )]
    WrongEncryptedDescriptorMagicNo(String),
    /// Wrong Encrypted Descriptor Version
    #[error("Wrong Version: Encrypted descriptor is the wrong version. The version byte was: {0}")]
    WrongEncryptedDescriptorVersion(u8),
    #[error("Wrong Descriptor: Wallet supports only taproot descriptor")]
    WrongDescriptor,
    /// Upgrade unnecessary
    #[error("Descriptor does not need to be upgraded")]
    UpgradeUnnecessary,
    /// Wrong network
    #[error("Address provided is on the wrong network!")]
    WrongNetwork,
    #[error("Wrong Psbt: PSBT hexadecimal cannot be parsed")]
    WrongPsbt,
    /// Wrong Encrypted Descriptor Format
    #[error("Insufficient satoshis to create funding wallet. Minimum: {0} sats")]
    InsufficientFundSats(u64),
    /// Drain wallet unable to finalize PSBT
    #[error("Drain wallet was unable to finalize PSBT")]
    DrainWalletUnfinalizedPsbt,
    /// Drain wallet was unable to find tx details
    #[error("No wallet transaction details were found when draining wallet")]
    DrainWalletNoTxDetails,
    /// BitMask Core Bitcoin Keys error
    #[error(transparent)]
    BitcoinKeysError(#[from] BitcoinKeysError),
    /// BitMask Core Bitcoin Payment error
    #[error(transparent)]
    BitcoinPaymentError(#[from] BitcoinPaymentError),
    /// BitMask Core Bitcoin Psbt error
    #[error(transparent)]
    BitcoinPsbtError(#[from] BitcoinPsbtError),
    /// BitMask Core Bitcoin Wallet error
    #[error(transparent)]
    BitcoinWalletError(#[from] BitcoinWalletError),
    /// hex decode error
    #[error(transparent)]
    HexDecodeError(#[from] hex::FromHexError),
    /// Wallet descriptor encrypt/decrypt (XChaCha20-Poly1305 + bincode)
    #[error(transparent)]
    WalletEncryptError(#[from] WalletEncryptError),
    /// BDK create-tx error
    #[error("BDK create tx: {0}")]
    BdkCreateTxError(String),
    /// BDK signer error
    #[error("BDK sign: {0}")]
    BdkSignError(String),
    /// BDK fee-bump error
    #[error("BDK fee bump: {0}")]
    BdkFeeBumpError(String),
    /// BDK esplora error
    #[error(transparent)]
    BdkEsploraError(#[from] bdk_esplora::esplora_client::Error),
    /// Bitcoin address parse
    #[error(transparent)]
    BitcoinAddressError(#[from] bitcoin::address::ParseError),
    /// PSBT extract
    #[error("PSBT extract tx: {0}")]
    BitcoinPsbtExtractError(String),
    /// Txid parse error
    #[error(transparent)]
    TxidParseError(#[from] bitcoin::hex::HexToArrayError),
    #[error("Password hashed with Argon2id: {0}")]
    HashPasswordError(String),
    #[error("Shared key hash must be of fixed size: {0}")]
    SharedKeyError(String),
    #[error("No fee present on tx")]
    NoFeePresent,
    #[error("Fee low. Please increase the fee rate and try again.")]
    FeeLow,
    #[error("Failed to build esplora client: {0}")]
    BuildEsploraError(String),
}

/// Bitcoin Wallet Operations
const BITMASK_ARGON2_SALT: &[u8] = b"DIBA BitMask Password Hash"; // Never change this

/// Convert product `f32` sat/vB to rust-bitcoin 0.32 `FeeRate`.
pub fn fee_rate_from_sat_vb(sat_vb: f32) -> FeeRate {
    let v = if sat_vb.is_finite() && sat_vb > 0.0 {
        sat_vb.ceil() as u64
    } else {
        1
    };
    FeeRate::from_sat_per_vb(v.max(1)).expect("non-zero sat/vB is a valid FeeRate")
}

fn product_balance(balance: bdk_wallet::Balance) -> Balance {
    Balance {
        immature: balance.immature.to_sat(),
        trusted_pending: balance.trusted_pending.to_sat(),
        untrusted_pending: balance.untrusted_pending.to_sat(),
        confirmed: balance.confirmed.to_sat(),
    }
}

fn product_block_time(pos: ChainPosition<ConfirmationBlockTime>) -> Option<BlockTime> {
    match pos {
        ChainPosition::Confirmed { anchor, .. } => Some(BlockTime {
            height: anchor.block_id.height,
            timestamp: anchor.confirmation_time,
        }),
        ChainPosition::Unconfirmed { .. } => None,
    }
}

pub fn hash_password(password: &SecretString) -> Result<SecretString, BitcoinError> {
    use argon2::{Algorithm, Params, Version};

    let mut output_key_material = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default())
        .hash_password_into(
            password.0.as_bytes(),
            BITMASK_ARGON2_SALT,
            &mut output_key_material,
        )
        .map_err(|e| BitcoinError::HashPasswordError(e.to_string()))?;

    let hash = SecretString(hex::encode(output_key_material));
    output_key_material.zeroize();

    Ok(hash)
}

pub async fn decrypt_wallet(
    hash: &SecretString,
    encrypted_descriptors: &SecretString,
    seed_password: &SecretString,
) -> Result<DecryptedWalletData, BitcoinError> {
    let mut shared_key: [u8; 32] = match hex::decode(&hash.0)?.try_into() {
        Ok(key) => key,
        Err(e) => return Err(BitcoinError::SharedKeyError(format!("{:?}", e))),
    };
    let encrypted_descriptors: Vec<u8> = hex::decode(&encrypted_descriptors.0)?;
    let (version_prefix, encrypted_descriptors) = encrypted_descriptors.split_at(5);

    if !version_prefix.starts_with(&DIBA_MAGIC_NO) {
        return Err(BitcoinError::WrongEncryptedDescriptorMagicNo(format!(
            "{version_prefix:#?}"
        )));
    }

    if version_prefix[4] != DIBA_DESCRIPTOR_VERSION {
        return Err(BitcoinError::WrongEncryptedDescriptorVersion(
            version_prefix[4],
        ));
    }

    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors.to_owned())?;

    let decrypted_wallet_data: DecryptedWalletData =
        decrypt_owned(&encrypted_message, &shared_key)?;

    shared_key.zeroize();

    let mnemonic_phrase = &SecretString(decrypted_wallet_data.mnemonic.clone());
    let decrypted_wallet_data = save_mnemonic(mnemonic_phrase, seed_password).await?;

    Ok(decrypted_wallet_data)
}

pub async fn upgrade_wallet(
    hash: &SecretString,
    encrypted_descriptors: &SecretString,
    seed_password: &SecretString,
) -> Result<SecretString, BitcoinError> {
    let shared_key: [u8; 32] = match hex::decode(&hash.0)?.try_into() {
        Ok(key) => key,
        Err(e) => return Err(BitcoinError::SharedKeyError(format!("{:?}", e))),
    };
    let encrypted_descriptors: Vec<u8> = hex::decode(&encrypted_descriptors.0)?;
    let encrypted_message = EncryptedMessage::deserialize(encrypted_descriptors)?;

    match decrypt_owned::<DecryptedWalletData>(&encrypted_message, &shared_key) {
        Ok(_data) => Err(BitcoinError::UpgradeUnnecessary),
        Err(_err) => {
            // If there's a deserialization error, attempt to recover just the mnemonic.
            let recovered_wallet_data: EncryptedWalletDataV04 =
                decrypt_owned(&encrypted_message, &shared_key)?;

            // println!("Recovered wallet data: {recovered_wallet_data:?}"); // Keep commented out for security
            // todo!("Add later version migrations here");

            let upgraded_descriptor = encrypt_wallet(
                &SecretString(recovered_wallet_data.mnemonic),
                hash,
                seed_password,
            )
            .await?;

            Ok(upgraded_descriptor)
        }
    }
}

pub fn versioned_descriptor(encrypted_message: EncryptedMessage) -> SecretString {
    let mut descriptor_data = DIBA_DESCRIPTOR.to_vec();
    let mut encrypted_descriptors = encrypted_message.serialize();
    descriptor_data.append(&mut encrypted_descriptors);

    let encrypted = SecretString(hex::encode(&descriptor_data));

    descriptor_data.zeroize();
    encrypted_descriptors.zeroize();
    encrypted
}

pub async fn new_wallet(
    hash: &SecretString,
    seed_password: &SecretString,
) -> Result<SecretString, BitcoinError> {
    let mut shared_key: [u8; 32] = match hex::decode(&hash.0)?.try_into() {
        Ok(key) => key,
        Err(e) => return Err(BitcoinError::SharedKeyError(format!("{:?}", e))),
    };

    let wallet_data = new_mnemonic(seed_password).await?;
    let encrypted_message = encrypt_owned(&wallet_data, &shared_key)?;
    let encrypted_descriptors = versioned_descriptor(encrypted_message);

    shared_key.zeroize();

    Ok(encrypted_descriptors)
}

pub async fn encrypt_wallet(
    mnemonic_phrase: &SecretString,
    hash: &SecretString,
    seed_password: &SecretString,
) -> Result<SecretString, BitcoinError> {
    let shared_key: [u8; 32] = match hex::decode(&hash.0)?.try_into() {
        Ok(key) => key,
        Err(e) => return Err(BitcoinError::SharedKeyError(format!("{:?}", e))),
    };

    let wallet_data = save_mnemonic(mnemonic_phrase, seed_password).await?;
    let encrypted_message = encrypt_owned(&wallet_data, &shared_key)?;
    let encrypted_descriptors = versioned_descriptor(encrypted_message);

    Ok(encrypted_descriptors)
}

pub async fn get_wallet_data(
    descriptor: &SecretString,
    change_descriptor: Option<&SecretString>,
) -> Result<WalletData, BitcoinError> {
    info!("get_wallet_data");

    let wallet = get_wallet(descriptor, change_descriptor).await?;
    sync_wallet(&wallet).await?;

    let address = {
        let mut w = wallet.lock().await;
        crate::bitcoin::wallet::next_unused_receive_address(&mut w)
    };
    info!(format!("address: {address}"));
    let balance = product_balance(wallet.lock().await.balance());
    info!(format!("balance: {balance:?}"));
    let utxos: Vec<String> = wallet
        .lock()
        .await
        .list_unspent()
        .map(|x| x.outpoint.to_string())
        .collect();
    trace!(format!("unspent: {utxos:#?}"));

    let mut transactions: Vec<WalletTransaction> = {
        let w = wallet.lock().await;
        w.transactions()
            .filter_map(|wtx| {
                let tx = wtx.tx_node.tx.as_ref();
                let vsize = tx.vsize().max(1);
                let fee = w.calculate_fee(tx).ok().map(|a| a.to_sat())?;
                let (sent, received) = w.sent_and_received(tx);
                let confirmation_time = product_block_time(wtx.chain_position);
                Some(WalletTransaction {
                    txid: tx.compute_txid(),
                    received: received.to_sat(),
                    sent: sent.to_sat(),
                    fee: Some(fee),
                    confirmed: confirmation_time.is_some(),
                    confirmation_time,
                    vsize,
                    fee_rate: fee as f32 / vsize as f32,
                })
            })
            .collect()
    };
    trace!(format!("transactions: {transactions:#?}"));

    transactions.sort_by(|a, b| {
        b.confirmation_time
            .as_ref()
            .map(|t| t.height)
            .cmp(&a.confirmation_time.as_ref().map(|t| t.height))
    });

    trace!(format!("transactions: {transactions:#?}"));

    Ok(WalletData {
        address,
        balance,
        transactions,
        utxos,
    })
}

pub async fn get_new_address(
    descriptor: &SecretString,
    change_descriptor: Option<&SecretString>,
) -> Result<String, BitcoinError> {
    info!("get_new_address");

    let wallet = get_wallet(descriptor, change_descriptor).await?;
    let address = {
        let mut w = wallet.lock().await;
        crate::bitcoin::wallet::reveal_next_receive_address(&mut w)
    };
    info!(format!("address: {address}"));
    Ok(address)
}

pub async fn validate_address(address: &Address<NetworkUnchecked>) -> Result<(), BitcoinError> {
    let network = *NETWORK.read().await;
    match network {
        Network::Signet => {
            if !address.is_valid_for_network(Network::Signet)
                && !address.is_valid_for_network(Network::Testnet)
            {
                return Err(BitcoinError::WrongNetwork);
            }
        }
        other => {
            if !address.is_valid_for_network(other) {
                return Err(BitcoinError::WrongNetwork);
            }
        }
    }
    Ok(())
}

async fn parse_checked_address(s: &str) -> Result<Address, BitcoinError> {
    let unchecked = Address::from_str(s)?;
    validate_address(&unchecked).await?;
    Ok(unchecked.assume_checked())
}

pub async fn send_sats(
    descriptor: &SecretString,
    change_descriptor: &SecretString,
    destination: &str, // bip21 uri or address
    amount: u64,
    broadcast: bool,
    fee_rate: Option<f32>,
) -> Result<TransactionData, BitcoinError> {
    let wallet = get_wallet(descriptor, Some(change_descriptor)).await?;
    let fee_rate = fee_rate.map(fee_rate_from_sat_vb);

    let details = match payjoin::Uri::try_from(destination) {
        Ok(uri) => {
            let uri = uri.assume_checked();
            let address = parse_checked_address(&uri.address().to_string()).await?;
            if let Ok(pj_uri) = uri.check_pj_supported() {
                create_payjoin(
                    vec![SatsInvoice {
                        address: address.clone().into_unchecked(),
                        amount,
                    }],
                    &wallet,
                    fee_rate,
                    pj_uri,
                )
                .await?
            } else {
                create_transaction(
                    vec![SatsInvoice {
                        address: address.into_unchecked(),
                        amount,
                    }],
                    &wallet,
                    broadcast,
                    fee_rate,
                )
                .await?
            }
        }
        _ => {
            let address = parse_checked_address(destination).await?;
            create_transaction(
                vec![SatsInvoice {
                    address: address.into_unchecked(),
                    amount,
                }],
                &wallet,
                broadcast,
                fee_rate,
            )
            .await?
        }
    };

    let vsize = match &details.transaction {
        Some(tx_details) => tx_details.vsize(),
        None => 1,
    };

    let fee_rate = match details.fee {
        Some(fee) => fee,
        None => return Err(BitcoinError::NoFeePresent),
    };

    let fee_rate = fee_rate as f32 / vsize as f32;

    Ok(TransactionData {
        details,
        vsize,
        fee_rate,
    })
}

pub async fn fund_vault(
    btc_descriptor_xprv: &SecretString,
    btc_change_descriptor_xprv: &SecretString,
    rgb_address: &str,
    broadcast: bool,
    fee_rate: Option<f32>,
) -> Result<FundVaultDetails, BitcoinError> {
    let wallet = get_wallet(btc_descriptor_xprv, Some(btc_change_descriptor_xprv)).await?;
    let fee_rate = fee_rate.map(fee_rate_from_sat_vb);

    let balance = product_balance(wallet.lock().await.balance());
    if balance.confirmed < MIN_FUNDS_SATS {
        return Err(BitcoinError::InsufficientFundSats(MIN_FUNDS_SATS));
    };

    let rgb_address = parse_checked_address(rgb_address).await?;
    let fee_address = Address::from_str(FEE_ADDRESS)?.assume_checked();

    let mut rng = StdRng::from_entropy();
    let dust_amount = rng.gen_range(589..1515);
    let fee_amount = rng.gen_range(10_101..11_511);

    let invoice = SatsInvoice {
        address: rgb_address.into_unchecked(),
        amount: dust_amount,
    };

    let mut invoices = vec![invoice];

    if *NETWORK.read().await == Network::Bitcoin {
        let fee = SatsInvoice {
            address: fee_address.into_unchecked(),
            amount: fee_amount,
        };
        invoices.push(fee);
    }

    let asset_tx_details = create_transaction(invoices, &wallet, broadcast, fee_rate).await?;

    let asset_txid = asset_tx_details.txid;

    info!(format!("asset txid: {asset_txid}"));
    info!(format!("asset details: {asset_tx_details:?}"));

    let rgb_output = if broadcast {
        asset_tx_details.transaction.as_ref().and_then(|tx| {
            tx.output
                .iter()
                .enumerate()
                .map(|(i, _)| format!("{asset_txid}:{i}"))
                .collect::<Vec<String>>()
                .first()
                .map(|o| o.to_owned())
        })
    } else {
        None
    };

    let cost = dust_amount + fee_amount + 2336;
    let fund_fee = asset_tx_details.fee.map(|f| f as u32 + cost as u32);

    Ok(FundVaultDetails {
        rgb_output,
        is_funded: broadcast,
        fund_txid: Some(asset_txid.to_string()),
        fund_fee,
    })
}

fn utxo_string(utxo: &LocalOutput) -> String {
    utxo.outpoint.to_string()
}

pub async fn get_assets_vault(
    rgb_descriptor_xpub: &SecretString,
) -> Result<FundVaultDetails, BitcoinError> {
    let rgb_wallet = get_wallet(rgb_descriptor_xpub, None).await?;

    let rgb_utxos: Vec<LocalOutput> = rgb_wallet.lock().await.list_unspent().collect();
    debug!(format!("RGB UTXOs: {rgb_utxos:#?}"));

    let mut rgb_utxos: Vec<String> = rgb_utxos.iter().map(utxo_string).collect();
    rgb_utxos.sort();

    let rgb_output = rgb_utxos.pop();

    let is_funded = rgb_output.is_some();

    Ok(FundVaultDetails {
        rgb_output,
        is_funded,
        fund_txid: None,
        fund_fee: None,
    })
}

pub async fn sign_psbt_file(request: SignPsbtRequest) -> Result<SignedPsbtResponse, BitcoinError> {
    let SignPsbtRequest { psbt, descriptors } = request;
    let final_psbt = Psbt::from_str(&psbt).map_err(|_| BitcoinError::WrongPsbt)?;

    let mut wallets = vec![];
    for descriptor in descriptors {
        let wallet = get_wallet(&descriptor, None).await?;
        wallets.push(wallet);
    }

    let psbt_signed = multi_sign_psbt(wallets, final_psbt).await?;
    let psbt_b64 = psbt_signed.to_string();
    Ok(SignedPsbtResponse {
        sign: true,
        psbt: psbt_b64,
    })
}

pub async fn publish_psbt_file(
    request: PublishPsbtRequest,
) -> Result<SignedPsbtResponse, BitcoinError> {
    let PublishPsbtRequest { psbt } = request;
    let final_psbt = Psbt::from_str(&psbt).map_err(|_| BitcoinError::WrongPsbt)?;

    publish_psbt(final_psbt.clone()).await?;
    let psbt_b64 = final_psbt.to_string();
    Ok(SignedPsbtResponse {
        sign: true,
        psbt: psbt_b64,
    })
}

pub async fn sign_and_publish_psbt_file(
    request: SignPsbtRequest,
) -> Result<PublishedPsbtResponse, BitcoinError> {
    let SignPsbtRequest { psbt, descriptors } = request;
    let final_psbt = Psbt::from_str(&psbt).map_err(|_| BitcoinError::WrongPsbt)?;

    let mut wallets = vec![];
    for descriptor in descriptors {
        let wallet = get_wallet(&descriptor, None).await?;
        wallets.push(wallet);
    }

    let sign = multi_sign_and_publish_psbt(wallets, final_psbt).await?;
    let resp = match sign.transaction {
        Some(tx) => PublishedPsbtResponse {
            sign: true,
            txid: tx.compute_txid().to_string(),
        },
        _ => PublishedPsbtResponse {
            sign: false,
            txid: String::new(),
        },
    };

    Ok(resp)
}

pub async fn publish_psbt_file_bp(
    request: PublishPsbtRequest,
) -> Result<SignedPsbtResponse, BitcoinError> {
    let PublishPsbtRequest { psbt } = request;
    let final_psbt = Psbt::from_str(&psbt).map_err(|_| BitcoinError::WrongPsbt)?;
    let tx_hex = &serialize(
        &final_psbt
            .clone()
            .extract_tx()
            .map_err(|e| BitcoinError::BitcoinPsbtExtractError(e.to_string()))?,
    )
    .to_hex();
    debug!(format!("tx: {tx_hex}"));
    let tx = Tx::from_str(tx_hex).map_err(|_| BitcoinError::WrongPsbt)?;

    let esplora_url = BITCOIN_EXPLORER_API.read().await.to_string();
    let esplora_client = esplora::Builder::new(&esplora_url)
        .build_async()
        .map_err(|e| BitcoinError::BuildEsploraError(e.to_string()))?;

    esplora_client.broadcast(&tx).await.map_err(|op| {
        BitcoinError::BitcoinPsbtError(BitcoinPsbtError::CouldNotBroadcastPsbt(op.to_string()))
    })?;

    let psbt_b64 = final_psbt.to_string();
    Ok(SignedPsbtResponse {
        sign: true,
        psbt: psbt_b64,
    })
}

pub async fn sign_and_publish_psbt_file_bp(
    request: SignPsbtRequest,
) -> Result<PublishedPsbtResponse, BitcoinError> {
    let SignPsbtRequest { psbt, descriptors } = request;
    let final_psbt = Psbt::from_str(&psbt).map_err(|_| BitcoinError::WrongPsbt)?;

    let mut wallets = vec![];
    for descriptor in descriptors {
        let wallet = get_wallet(&descriptor, None).await?;
        wallets.push(wallet);
    }

    let psbt_signed = multi_sign_psbt(wallets, final_psbt).await?;
    let tx_hex = &serialize(
        &psbt_signed
            .clone()
            .extract_tx()
            .map_err(|e| BitcoinError::BitcoinPsbtExtractError(e.to_string()))?,
    )
    .to_hex();
    let tx = Tx::from_str(tx_hex).map_err(|_| BitcoinError::WrongPsbt)?;

    let esplora_url = BITCOIN_EXPLORER_API.read().await.to_string();
    let esplora_client = esplora::Builder::new(&esplora_url)
        .build_async()
        .map_err(|e| BitcoinError::BuildEsploraError(e.to_string()))?;

    debug!(format!("tx for broadcast: {tx_hex}"));
    esplora_client
        .broadcast(&tx)
        .await
        .map_err(|op| BitcoinError::BitcoinPsbtError(BitcoinPsbtError::Esplora(op)))?;

    Ok(PublishedPsbtResponse {
        sign: true,
        txid: tx.txid().to_string(),
    })
}

pub async fn drain_wallet(
    destination: &str,
    descriptor: &SecretString,
    change_descriptor: Option<&SecretString>,
    fee_rate: Option<f32>,
) -> Result<TransactionData, BitcoinError> {
    let address = parse_checked_address(destination).await?;
    debug!(format!("Create drain wallet tx to: {address:#?}"));

    let wallet = get_wallet(descriptor, change_descriptor).await?;
    sync_wallet(&wallet).await?;

    let fee_rate = fee_rate.map(fee_rate_from_sat_vb);

    let mut psbt = {
        let mut locked_wallet = wallet.lock().await;
        let mut builder = locked_wallet.build_tx();
        if let Some(fee_rate) = fee_rate {
            builder.fee_rate(fee_rate);
        }
        builder.drain_wallet();
        builder.drain_to(address.script_pubkey());
        builder
            .finish()
            .map_err(|e| BitcoinError::BdkCreateTxError(e.to_string()))?
    };

    debug!("Signing PSBT...");
    let finalized = wallet
        .lock()
        .await
        .sign(&mut psbt, SignOptions::default())
        .map_err(|e| BitcoinError::BdkSignError(e.to_string()))?;

    if !finalized {
        return Err(BitcoinError::DrainWalletUnfinalizedPsbt);
    }
    debug!(format!("Finalized: {finalized}"));

    let fee_sats = psbt
        .fee()
        .map(|a| a.to_sat())
        .map_err(|_| BitcoinError::NoFeePresent)?;
    let blockchain = get_blockchain().await?;
    let tx = psbt
        .extract_tx()
        .map_err(|e| BitcoinError::BitcoinPsbtExtractError(e.to_string()))?;
    let txid = tx.compute_txid();
    blockchain.broadcast(&tx).await?;
    let fetched = blockchain.get_tx(&txid).await?;

    if let Some(transaction) = fetched.clone() {
        let sent = transaction
            .output
            .iter()
            .fold(0, |sum, output| output.value.to_sat() + sum);

        let details = TransactionDetails {
            transaction: fetched,
            txid: transaction.compute_txid(),
            received: 0,
            sent,
            fee: Some(fee_sats),
            confirmation_time: None,
        };

        info!(format!(
            "Drain wallet transaction submitted with details: {details:#?}"
        ));

        let vsize = match &details.transaction {
            Some(tx_details) => tx_details.vsize(),
            None => 1,
        };

        let fee_rate = fee_sats as f32 / vsize as f32;

        Ok(TransactionData {
            details,
            vsize,
            fee_rate,
        })
    } else {
        Err(BitcoinError::DrainWalletNoTxDetails)
    }
}

pub async fn bump_fee(
    txid: String,
    fee_rate: f32,
    descriptor: &SecretString,
    change_descriptor: Option<&SecretString>,
    broadcast: bool,
) -> Result<TransactionData, BitcoinError> {
    let txid = Txid::from_str(&txid)?;

    let wallet = get_wallet(descriptor, change_descriptor).await?;

    if broadcast {
        sync_wallet(&wallet).await?;
    }

    let mut psbt = {
        let mut wallet_lock = wallet.lock().await;
        let mut builder = wallet_lock
            .build_fee_bump(txid)
            .map_err(|e| BitcoinError::BdkFeeBumpError(e.to_string()))?;
        builder.fee_rate(fee_rate_from_sat_vb(fee_rate));
        builder
            .finish()
            .map_err(|e| BitcoinError::BdkCreateTxError(e.to_string()))?
    };

    let _finalized = wallet
        .lock()
        .await
        .sign(&mut psbt, SignOptions::default())
        .map_err(|e| BitcoinError::BdkSignError(e.to_string()))?;
    let fee_sats = psbt.fee().ok().map(|a| a.to_sat());
    let tx = psbt
        .extract_tx()
        .map_err(|e| BitcoinError::BitcoinPsbtExtractError(e.to_string()))?;

    if broadcast {
        let blockchain = get_blockchain().await?;
        blockchain.broadcast(&tx).await?;
    }

    let sent = tx
        .output
        .iter()
        .fold(0, |sum, output| output.value.to_sat() + sum);

    let txid = tx.compute_txid();
    let vsize = tx.vsize();

    let details = TransactionDetails {
        txid,
        transaction: Some(tx),
        received: 0,
        sent,
        fee: fee_sats,
        confirmation_time: None,
    };

    Ok(TransactionData {
        details,
        vsize,
        fee_rate,
    })
}
