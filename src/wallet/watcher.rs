use crate::bp::models::BtcAccount;
use crate::constants::{BITCOIN_EXPLORER_API, bp_network_from_str, get_network};
use crate::debug;
use crate::rgb::contract::ExportContractError;
use crate::rgb::data::{RGBStock, RGBStockCrossStore};
use crate::rgb::models::{RgbAccount, UtxoKit};
use crate::rgb::prefetch::BitmaskResolver;
use crate::structs::{
    NextAddressResponse, NextUtxoResponse, WatcherRequest, WatcherResponse, WatcherStatusResponse,
};
use crate::wallet::data::{
    AccountCrossStore, BPWallet, CrossStoreAsync, CrossStoreError, SyncProvider,
};
use crate::wallet::interop::{
    BpInteropError, DerivationType, WalletTerminal, WalletType, XpubKeyBp,
};
use crate::wallet::prefetch::PrefetchSync;
use bpstd::psbt::PsbtConstructor;
use bpstd::{Network, Outpoint, ScriptPubkey};
use bpwallet::{TxStatus, WalletUtxo, XpubDerivable};
use nonasync::persistence::CloneNoPersistence;
use rgb::{RgbDescr, TapretKey, WpkhDescr};
use rgbcore::ChainNet;
use std::collections::BTreeMap;
use std::str::FromStr;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum WatcherError {
    /// some request data is missing. {0:?}
    Invalid(BTreeMap<String, String>),
    /// I/O or connectivity error. {0}
    IO(CrossStoreError),
    /// rgb store error. {0}
    Store(String),
    /// rgb stock error. {0}
    Stock(String),
    /// retrieve server data error. {0}
    Server(crate::wallet::server::ServerErrors),
    /// wrong network
    WrongNetwork,
    /// wrong asset. {0}
    WrongAsset(String),
    /// descriptor parse error. {0}
    WrongDesc(BpInteropError),
    /// create watcher fail. {0}
    Create(String),
    /// next address retrieve fail. {0}
    NextAddress(String),
    /// block esplorer sync error. {0}
    Explorer(String),
    /// contract export fail. {0}
    Export(ExportContractError),
}

pub async fn create_watcher(
    sk: &str,
    request: WatcherRequest,
) -> Result<WatcherResponse, WatcherError> {
    let WatcherRequest {
        btc_xpub,
        rgb_xpub,
        recover,
        ..
    } = request;

    let network =
        bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;
    let chain_net = match network {
        Network::Mainnet => ChainNet::BitcoinMainnet,
        Network::Regtest => ChainNet::BitcoinRegtest,
        Network::Signet => ChainNet::BitcoinSignet,
        Network::Testnet3 => ChainNet::BitcoinTestnet3,
        Network::Testnet4 => ChainNet::BitcoinTestnet4,
    };

    let deriv_type = if recover {
        DerivationType::All
    } else {
        DerivationType::RGB
    };

    let interop_key = XpubKeyBp::new(&rgb_xpub, network, WalletType::Taproot, deriv_type)
        .map_err(WatcherError::WrongDesc)?;
    let taproot = TapretKey::with_key(interop_key.xpub);
    let descr = RgbDescr::from(taproot);

    let rgb_wallet = BPWallet::new_layer1(descr, network);

    let mut account = RgbAccount::new(rgb_wallet, chain_net);
    let mut provider = if recover {
        AccountCrossStore::rgb_recover(sk, &get_network().await)
            .map_err(|err| WatcherError::Create(err.to_string()))?
    } else {
        AccountCrossStore::rgb(sk, &get_network().await)
            .map_err(|err| WatcherError::Create(err.to_string()))?
    };

    account
        .sync(&mut provider)
        .map_err(|err| WatcherError::Store(err.to_string()))?;
    provider.persist().await.map_err(WatcherError::IO)?;

    let mut provider = RGBStockCrossStore::with(sk, &get_network().await)
        .map_err(|err| WatcherError::Create(err.to_string()))?;

    provider
        .fetch()
        .await
        .map_err(|err| WatcherError::Stock(err.to_string()))?;

    let mut rgb_stock =
        RGBStock::get(provider.clone()).map_err(|err| WatcherError::Stock(err.to_string()))?;
    rgb_stock
        .sync(&mut provider)
        .map_err(|err| WatcherError::Store(err.to_string()))?;

    let deriv_type = if recover {
        DerivationType::All
    } else {
        DerivationType::Bitcoin
    };

    let wallet_ty = if recover && btc_xpub.contains("tr(") {
        WalletType::Taproot
    } else {
        WalletType::SegWit
    };

    let interop_key = XpubKeyBp::new(&btc_xpub, network, wallet_ty, deriv_type)
        .map_err(WatcherError::WrongDesc)?;

    let mut descr = RgbDescr::Wpkh(WpkhDescr::with_key(interop_key.xpub.clone()));
    if recover && btc_xpub.contains("tr(") {
        descr = RgbDescr::TapretKey(TapretKey::with_key(interop_key.xpub));
    }

    let btc_wallet = BPWallet::new_layer1(descr, network);

    let mut account = BtcAccount::new(btc_wallet, chain_net);
    let mut provider = if recover {
        AccountCrossStore::btc_recover(sk, &get_network().await)
            .map_err(|err| WatcherError::Create(err.to_string()))?
    } else {
        AccountCrossStore::btc(sk, &get_network().await)
            .map_err(|err| WatcherError::Create(err.to_string()))?
    };

    account
        .sync(&mut provider)
        .map_err(|err| WatcherError::Store(err.to_string()))?;
    provider.persist().await.map_err(WatcherError::IO)?;

    Ok(WatcherResponse {
        network: get_network().await,
        name: "default".to_string(),
        migrate: false,
    })
}

pub async fn destroy_watcher(sk: &str, recover: bool) -> Result<(), WatcherError> {
    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let mut rgb_account = if recover {
        AccountCrossStore::rgb_recover(sk, &get_network().await)
            .map_err(|err| WatcherError::Create(err.to_string()))?
    } else {
        AccountCrossStore::rgb(sk, &get_network().await)
            .map_err(|err| WatcherError::Create(err.to_string()))?
    };

    let mut btc_account = if recover {
        AccountCrossStore::btc_recover(sk, &get_network().await)
            .map_err(|err| WatcherError::Create(err.to_string()))?
    } else {
        AccountCrossStore::btc(sk, &get_network().await)
            .map_err(|err| WatcherError::Create(err.to_string()))?
    };

    let mut rgb_stock = RGBStockCrossStore::with(sk, &get_network().await)
        .map_err(|err| WatcherError::Create(err.to_string()))?;

    btc_account.destroy().await.map_err(WatcherError::IO)?;
    rgb_account.destroy().await.map_err(WatcherError::IO)?;
    rgb_stock.destroy().await.map_err(WatcherError::IO)?;
    Ok(())
}

pub async fn check_watcher(sk: &str) -> Result<WatcherStatusResponse, WatcherError> {
    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let mut provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?;
    provider.fetch().await.map_err(WatcherError::IO)?;

    let created = BPWallet::get(provider).is_ok();

    Ok(WatcherStatusResponse {
        name: "default".to_string(),
        created,
    })
}

pub async fn watcher_prefetch(sk: &str, asset_id: &str) -> Result<(), WatcherError> {
    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let terminal = WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?;

    let mut provider = if terminal.is_bitcoin() {
        let mut provider =
            AccountCrossStore::btc(sk, &get_network().await).map_err(WatcherError::IO)?;
        provider.fetch().await.map_err(WatcherError::IO)?;

        let btc_account = BtcAccount::get(provider.clone())
            .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

        btc_account
            .fetch(&mut provider, false)
            .await
            .map_err(|err| WatcherError::Explorer(err.to_string()))?;

        provider
    } else {
        let mut provider =
            AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?;

        provider.fetch().await.map_err(WatcherError::IO)?;

        let rgb_account = RgbAccount::get(provider.clone())
            .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

        rgb_account
            .fetch(&mut provider, false)
            .await
            .map_err(|err| WatcherError::Explorer(err.to_string()))?;

        provider
    };

    provider.persist().await.map_err(WatcherError::IO)?;
    Ok(())
}

pub async fn watcher_next_address(
    sk: &str,
    asset_id: &str,
    shift: bool,
) -> Result<NextAddressResponse, WatcherError> {
    if asset_id.is_empty() {
        let errors = bmap! { "asset_id".to_string() => "parameter is required".to_string() };
        return Err(WatcherError::Invalid(errors));
    }

    watcher_prefetch(sk, asset_id).await?;

    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let terminal = WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?;

    let wallet_keychain = *terminal.keychain();

    let mut provider = if terminal.is_bitcoin() {
        AccountCrossStore::btc(sk, &get_network().await).map_err(WatcherError::IO)?
    } else {
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?
    };

    provider.fetch().await.map_err(WatcherError::IO)?;

    let mut wallet = BPWallet::get(provider.clone_no_persistence())
        .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

    let address = wallet.next_address(wallet_keychain, shift);
    let last_index = wallet.last_derivation_index(wallet_keychain);
    let keychain = format!("{wallet_keychain}/{last_index}");

    wallet.sync(&mut provider).expect("invalid wallet sync");
    provider.persist().await.map_err(WatcherError::IO)?;

    let next_address = NextAddressResponse {
        address: address.to_string(),
        network: get_network().await,
        keychain,
    };

    Ok(next_address)
}

pub async fn watcher_next_change_address(
    sk: &str,
    asset_id: &str,
    shift: bool,
) -> Result<NextAddressResponse, WatcherError> {
    if asset_id.is_empty() {
        let errors = bmap! { "asset_id".to_string() => "parameter is required".to_string() };
        return Err(WatcherError::Invalid(errors));
    }

    watcher_prefetch(sk, asset_id).await?;

    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let terminal = WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?;

    let wallet_keychain = terminal.clone().to_alt();

    let mut provider = if terminal.is_bitcoin() {
        AccountCrossStore::btc(sk, &get_network().await).map_err(WatcherError::IO)?
    } else {
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?
    };

    provider.fetch().await.map_err(WatcherError::IO)?;

    let mut wallet = BPWallet::get(provider.clone_no_persistence())
        .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

    let address = wallet.next_address(wallet_keychain, shift);
    let last_index = wallet.last_derivation_index(wallet_keychain);
    let keychain = format!("{wallet_keychain}/{last_index}");

    wallet.sync(&mut provider).expect("invalid wallet sync");
    provider.persist().await.map_err(WatcherError::IO)?;

    let next_address = NextAddressResponse {
        address: address.to_string(),
        network: get_network().await,
        keychain,
    };

    Ok(next_address)
}

pub async fn watcher_next_addresses(
    sk: &str,
    asset_id: &str,
    take: u8,
) -> Result<Vec<NextAddressResponse>, WatcherError> {
    if asset_id.is_empty() {
        let errors = bmap! { "asset_id".to_string() => "parameter is required".to_string() };
        return Err(WatcherError::Invalid(errors));
    }

    watcher_prefetch(sk, asset_id).await?;

    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let terminal = WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?;

    let mut provider = if terminal.is_bitcoin() {
        AccountCrossStore::btc(sk, &get_network().await).map_err(WatcherError::IO)?
    } else {
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?
    };
    provider.fetch().await.map_err(WatcherError::IO)?;

    let mut wallet = BPWallet::get(provider.clone())
        .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

    let wallet_keychain = *WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?
        .keychain();

    let mut next_addresses = vec![];

    let mut limit = 0;
    while limit <= take {
        let address = wallet.next_address(wallet_keychain, true);
        next_addresses.push(NextAddressResponse {
            address: address.to_string(),
            keychain: wallet_keychain.to_string(),
            network: get_network().await,
        });
        limit += 1;
    }

    wallet.sync(&mut provider).expect("invalid wallet sync");
    provider.persist().await.map_err(WatcherError::IO)?;

    Ok(next_addresses)
}

pub async fn watcher_next_utxo(sk: &str, asset_id: &str) -> Result<NextUtxoResponse, WatcherError> {
    if asset_id.is_empty() {
        let errors = bmap! { "asset_id".to_string() => "parameter is required".to_string() };
        return Err(WatcherError::Invalid(errors));
    }

    watcher_prefetch(sk, asset_id).await?;

    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let terminal = WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?;

    let wallet_keychain = *terminal.keychain();

    let is_bitcoin = terminal.is_bitcoin();

    let mut provider = if is_bitcoin {
        AccountCrossStore::btc(sk, &get_network().await).map_err(WatcherError::IO)?
    } else {
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?
    };

    provider.fetch().await.map_err(WatcherError::IO)?;

    let wallet = BPWallet::get(provider.clone_no_persistence())
        .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

    // Ensure the candidate UTXO is actually unspent by checking esplora.
    let rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    let client = rgb_resolver
        .explorer()
        .map_err(|e| WatcherError::Explorer(e.to_string()))?;

    let mut next_outpoint = none!();
    let mut next_outpoint_value = none!();

    // Iterate over vault UTXOs (preferring larger ones first) and pick the first truly
    // unspent outpoint. This prevents returning an already-spent vault output that is still
    // lingering in the local cache while the wallet sync completes.
    let mut utxos: Vec<_> = wallet
        .utxos()
        .filter(|u| u.terminal.keychain == wallet_keychain)
        .collect();

    debug!(format!("utxos utxos={:?}", utxos));
    // Prefer largest UTXO to maximise chances that the anchor tx has sufficient fee margin.
    utxos.sort_by_key(|a| std::cmp::Reverse(a.value));

    for utxo in utxos {
        let Outpoint { txid, vout } = utxo.outpoint;

        if let Some(status) = client
            .output_status(&txid, vout.into_u32().into())
            .await
            .map_err(|e| WatcherError::Explorer(e.to_string()))?
            && !status.spent
        {
            let out_str = utxo.outpoint.to_string();
            debug!(format!("utxos unspent, selecting out_str={:?}", out_str));
            next_outpoint = Some(out_str);
            next_outpoint_value = Some(utxo.value.into());
            break;
        }
    }

    Ok(NextUtxoResponse {
        utxo: next_outpoint,
        value: next_outpoint_value,
    })
}

pub async fn watcher_unspent_utxo(
    sk: &str,
    asset_id: &str,
    unanchored: bool,
) -> Result<NextUtxoResponse, WatcherError> {
    if asset_id.is_empty() {
        let errors = bmap! { "asset_id".to_string() => "parameter is required".to_string() };
        return Err(WatcherError::Invalid(errors));
    }

    watcher_prefetch(sk, asset_id).await?;

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(WatcherError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| WatcherError::Stock(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| WatcherError::Stock(err.to_string()))?;

    let terminal = WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?;

    let mut provider = if terminal.is_bitcoin() {
        AccountCrossStore::btc(sk, &get_network().await).map_err(WatcherError::IO)?
    } else {
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?
    };
    provider.fetch().await.map_err(WatcherError::IO)?;

    let mut rgb_account = RgbAccount::get(provider)
        .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

    let rgb_wallet = rgb_account.wallet.clone_no_persistence();
    let wallet_keychain = *WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?
        .keychain();

    let utxos: Vec<_> = rgb_wallet
        .utxos()
        .find(|x| x.terminal.keychain == wallet_keychain)
        .into_iter()
        .collect();

    let client = rgb_resolver.explorer().expect("esplora");
    let mut next_outpoints = vec![];
    for utxo in utxos {
        let Outpoint { txid, vout: index } = utxo.outpoint;
        if let Some(outpoint) = client
            .output_status(&txid, index.into_u32().into())
            .await
            .expect("esplora")
            && !outpoint.spent
            && wallet_keychain == utxo.terminal.keychain
        {
            next_outpoints.push(utxo.outpoint.to_string());
        }
    }

    let next_outpoint = if unanchored {
        let mut allocations = vec![];
        let contracts_list: Vec<rgbstd::info::ContractInfo> = rgb_stock
            .contracts()
            .map_err(|err| WatcherError::Stock(err.to_string()))?
            .collect();

        for contract in contracts_list {
            let contract_exported = crate::rgb::contract::get_contract(
                contract.id,
                &mut rgb_stock,
                &mut rgb_account,
                &mut rgb_resolver,
            )
            .await
            .map_err(WatcherError::Export)?;

            allocations.push(contract_exported.issue_utxo);
            allocations.extend(contract_exported.allocations.iter().map(|x| x.utxo.clone()));
        }

        let mut next_utxo = none!();
        for outpoint in next_outpoints {
            if !allocations.contains(&outpoint) {
                next_utxo = Some(outpoint);
                break;
            }
        }

        next_utxo
    } else {
        next_outpoints.first().cloned()
    };

    Ok(NextUtxoResponse {
        utxo: next_outpoint,
        value: None,
    })
}

pub async fn watcher_unspent_utxos(
    sk: &str,
    asset_id: &str,
    unanchored: bool,
) -> Result<Vec<UtxoKit>, WatcherError> {
    if asset_id.is_empty() {
        let errors = bmap! { "asset_id".to_string() => "parameter is required".to_string() };
        return Err(WatcherError::Invalid(errors));
    }

    watcher_prefetch(sk, asset_id).await?;

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(WatcherError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| WatcherError::Stock(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| WatcherError::Stock(err.to_string()))?;

    let terminal = WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?;

    let mut provider = if terminal.is_bitcoin() {
        AccountCrossStore::btc(sk, &get_network().await).map_err(WatcherError::IO)?
    } else {
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?
    };
    provider.fetch().await.map_err(WatcherError::IO)?;

    let mut rgb_account = RgbAccount::get(provider)
        .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

    let rgb_wallet = rgb_account.wallet.clone_no_persistence();
    let wallet_keychain = *WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?
        .keychain();

    let utxos: Vec<_> = rgb_wallet
        .utxos()
        .find(|x| x.terminal.keychain == wallet_keychain)
        .into_iter()
        .collect();

    let client = rgb_resolver.explorer().expect("esplora");
    let mut utxo_kits: Vec<UtxoKit> = vec![];
    for utxo in utxos {
        let Outpoint { txid, vout: index } = utxo.outpoint;
        if let Some(outpoint) = client
            .output_status(&txid, index.into_u32().into())
            .await
            .expect("esplora")
            && !outpoint.spent
            && wallet_keychain == utxo.terminal.keychain
        {
            utxo_kits.push(utxo.into());
        }
    }

    let next_outpoints = if unanchored {
        let mut allocations = vec![];
        let contracts_list: Vec<rgbstd::info::ContractInfo> = rgb_stock
            .contracts()
            .map_err(|err| WatcherError::Stock(err.to_string()))?
            .collect();

        for contract in contracts_list {
            let contract_exported = crate::rgb::contract::get_contract(
                contract.id,
                &mut rgb_stock,
                &mut rgb_account,
                &mut rgb_resolver,
            )
            .await
            .map_err(WatcherError::Export)?;
            allocations.extend(contract_exported.allocations.iter().map(|x| x.utxo.clone()));
        }

        let mut next_utxos = vec![];
        for kit in utxo_kits {
            let utxo = &kit.outpoint.to_string();
            if !allocations.iter().any(|x| x.contains(utxo)) {
                next_utxos.push(kit);
                break;
            }
        }

        next_utxos
    } else {
        utxo_kits
    };

    Ok(next_outpoints)
}

pub async fn watcher_anchored_utxos(
    sk: &str,
    asset_id: &str,
) -> Result<Vec<UtxoKit>, WatcherError> {
    if asset_id.is_empty() {
        let errors = bmap! { "asset_id".to_string() => "parameter is required".to_string() };
        return Err(WatcherError::Invalid(errors));
    }

    watcher_prefetch(sk, asset_id).await?;

    let mut rgb_resolver = BitmaskResolver {
        explorer_url: BITCOIN_EXPLORER_API.read().await.to_string(),
        ..default!()
    };

    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let mut rgb_stock_provider =
        RGBStockCrossStore::with(sk, &get_network().await).map_err(WatcherError::IO)?;

    rgb_stock_provider
        .fetch()
        .await
        .map_err(|err| WatcherError::Stock(err.to_string()))?;

    let mut rgb_stock = RGBStock::get(rgb_stock_provider.clone())
        .map_err(|err| WatcherError::Stock(err.to_string()))?;

    let mut provider =
        AccountCrossStore::rgb(sk, &get_network().await).map_err(WatcherError::IO)?;
    provider.fetch().await.map_err(WatcherError::IO)?;

    let mut rgb_account = RgbAccount::get(provider)
        .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

    let rgb_wallet = rgb_account.wallet.clone_no_persistence();
    let wallet_keychain = *WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?
        .keychain();

    let utxos: Vec<_> = rgb_wallet
        .utxos()
        .find(|x| x.terminal.keychain == wallet_keychain)
        .into_iter()
        .collect();

    let client = rgb_resolver.explorer().expect("esplora");
    let mut utxo_kits: Vec<UtxoKit> = vec![];
    for utxo in utxos {
        let Outpoint { txid, vout: index } = utxo.outpoint;
        if let Some(outpoint) = client
            .output_status(&txid, index.into_u32().into())
            .await
            .expect("esplora")
            && !outpoint.spent
            && wallet_keychain == utxo.terminal.keychain
        {
            utxo_kits.push(utxo.into());
        }
    }

    let mut allocations = vec![];
    let contracts_list: Vec<rgbstd::info::ContractInfo> = rgb_stock
        .contracts()
        .map_err(|err| WatcherError::Stock(err.to_string()))?
        .collect();

    for contract in contracts_list {
        let contract_exported = crate::rgb::contract::get_contract(
            contract.id,
            &mut rgb_stock,
            &mut rgb_account,
            &mut rgb_resolver,
        )
        .await
        .map_err(WatcherError::Export)?;

        allocations.extend(contract_exported.allocations.iter().map(|x| x.utxo.clone()));
    }

    let mut locked_utxos = vec![];
    for kit in utxo_kits {
        let utxo = &kit.outpoint.to_string();
        if allocations.iter().any(|x| x.contains(utxo)) {
            locked_utxos.push(kit);
            break;
        }
    }

    Ok(locked_utxos)
}

// Recover
pub async fn watcher_recover_utxos(
    sk: &str,
    asset_id: &str,
) -> Result<(RgbDescr<XpubDerivable>, Vec<(WalletUtxo, ScriptPubkey)>), WatcherError> {
    if asset_id.is_empty() {
        let errors = bmap! { "asset_id".to_string() => "parameter is required".to_string() };
        return Err(WatcherError::Invalid(errors));
    }

    watcher_inspect(sk, asset_id).await?;

    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let terminal = WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?;

    let mut provider = if terminal.is_bitcoin() {
        AccountCrossStore::btc_recover(sk, &get_network().await).map_err(WatcherError::IO)?
    } else {
        AccountCrossStore::rgb_recover(sk, &get_network().await).map_err(WatcherError::IO)?
    };

    provider.fetch().await.map_err(WatcherError::IO)?;

    let wallet = BPWallet::get(provider.clone_no_persistence())
        .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

    let utxo_kits: Vec<_> = wallet
        .utxos()
        .filter(|x| matches!(x.status, TxStatus::Mined(_)))
        .map(|x| x.to_owned())
        .collect();

    let mut result = Vec::with_capacity(utxo_kits.len());
    for utxo_kit in utxo_kits {
        if let Ok(item) = wallet.outpoint_by(utxo_kit.outpoint) {
            result.push(item);
        }
    }
    Ok((wallet.descriptor().clone(), result))
}

pub async fn watcher_inspect(sk: &str, asset_id: &str) -> Result<(), WatcherError> {
    bp_network_from_str(&get_network().await).map_err(|_| WatcherError::WrongNetwork)?;

    let terminal = WalletTerminal::from_str(asset_id)
        .map_err(|_| WatcherError::WrongAsset(asset_id.to_string()))?;

    let mut provider = if terminal.is_bitcoin() {
        let mut provider =
            AccountCrossStore::btc_recover(sk, &get_network().await).map_err(WatcherError::IO)?;
        provider.fetch().await.map_err(WatcherError::IO)?;

        let btc_account = BtcAccount::get(provider.clone())
            .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

        btc_account
            .fetch(&mut provider, true)
            .await
            .map_err(|err| WatcherError::Explorer(err.to_string()))?;

        provider
    } else {
        let mut provider =
            AccountCrossStore::rgb_recover(sk, &get_network().await).map_err(WatcherError::IO)?;

        provider.fetch().await.map_err(WatcherError::IO)?;

        let rgb_account = RgbAccount::get(provider.clone())
            .map_err(|err| WatcherError::IO(CrossStoreError::Fail(err.to_string())))?;

        rgb_account
            .fetch(&mut provider, true)
            .await
            .map_err(|err| WatcherError::Explorer(err.to_string()))?;

        provider
    };

    provider.persist().await.map_err(WatcherError::IO)?;
    Ok(())
}
