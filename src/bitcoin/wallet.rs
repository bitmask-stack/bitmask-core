use std::{collections::BTreeMap, sync::Arc};

use bdk_esplora::EsploraAsyncExt;
use bdk_esplora::esplora_client::{AsyncClient, Builder as EsploraBuilder};
use bdk_wallet::bitcoin::Network;
use bdk_wallet::{KeychainKind, Wallet};
use bitcoin::hashes::{Hash, sha256};
use futures::Future;
use once_cell::sync::Lazy;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};

use crate::{
    constants::{BITCOIN_EXPLORER_API, NETWORK},
    debug,
    structs::SecretString,
};

/// wasm `bdk_esplora` is `async` only (`build_async` needs the tokio feature).
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, Copy)]
pub struct WasmEsploraSleeper;

#[cfg(target_arch = "wasm32")]
impl bdk_esplora::esplora_client::Sleeper for WasmEsploraSleeper {
    type Sleep = std::future::Ready<()>;

    fn sleep(_dur: std::time::Duration) -> Self::Sleep {
        std::future::ready(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type EsploraAsyncClient = AsyncClient;
#[cfg(target_arch = "wasm32")]
pub type EsploraAsyncClient = AsyncClient<WasmEsploraSleeper>;

/// Esplora stop-gap used by BDK 0.28 `EsploraBlockchain::new(url, 1)`. Keep 1.
const ESPLORA_STOP_GAP: usize = 1;
const ESPLORA_PARALLEL_REQUESTS: usize = 1;

#[allow(clippy::result_large_err)] // BDK 1.x + esplora error graph; do not reshape
#[derive(Error, Debug)]
pub enum BitcoinWalletError {
    /// Unexpected key variant in get_descriptor
    #[error("Unexpected key variant in get_descriptor")]
    UnexpectedKey,
    /// BDK descriptor / wallet create
    #[error(transparent)]
    BdkDescriptorError(#[from] bdk_wallet::descriptor::error::Error),
    /// Esplora HTTP client
    #[error(transparent)]
    BdkEsploraError(#[from] bdk_esplora::esplora_client::Error),
    /// Applying an esplora update to a memory wallet
    #[error("BDK chain update: {0}")]
    BdkConnectError(String),
    /// Failed to build the BDK esplora client
    #[error("Failed to build esplora client: {0}")]
    BuildEsploraError(String),
}

pub type MemoryWallet = Arc<Mutex<Wallet>>;
type Wallets = BTreeMap<(String, Option<String>), MemoryWallet>;
type NetworkWallet = Arc<RwLock<Wallets>>;

#[derive(Default)]
struct Networks {
    bitcoin: NetworkWallet,
    testnet: NetworkWallet,
    testnet4: NetworkWallet,
    signet: NetworkWallet,
    regtest: NetworkWallet,
}

static BDK: Lazy<Networks> = Lazy::new(Networks::default);

fn wallets_for_network(network: Network) -> Result<NetworkWallet, BitcoinWalletError> {
    match network {
        Network::Bitcoin => Ok(BDK.bitcoin.clone()),
        Network::Testnet => Ok(BDK.testnet.clone()),
        Network::Testnet4 => Ok(BDK.testnet4.clone()),
        Network::Signet => Ok(BDK.signet.clone()),
        Network::Regtest => Ok(BDK.regtest.clone()),
    }
}

async fn access_network_wallets<U, F, Fut>(
    network: Network,
    mut f: F,
) -> Result<(), BitcoinWalletError>
where
    U: 'static + Send,
    F: 'static + FnMut(NetworkWallet) -> Fut + Send,
    Fut: 'static + Future<Output = Result<U, BitcoinWalletError>> + Send,
{
    f(wallets_for_network(network)?).await?;
    Ok(())
}

pub async fn get_wallet(
    descriptor: &SecretString,
    change_descriptor: Option<&SecretString>,
) -> Result<MemoryWallet, BitcoinWalletError> {
    let descriptor_key = format!("{descriptor:?}{change_descriptor:?}");
    let key: String = sha256::Hash::hash(descriptor_key.as_bytes()).to_string();

    let network_lock = NETWORK.read().await;
    let network = network_lock.to_owned();
    drop(network_lock);

    let wallets = wallets_for_network(network)?;

    let wallets_lock = wallets.read().await;
    let wallets_ref = wallets_lock.get(&(key.clone(), None));
    if let Some(wallets) = wallets_ref {
        return Ok(wallets.clone());
    }
    drop(wallets_lock);

    let external = descriptor.0.clone();
    let created = match change_descriptor {
        Some(change) => {
            let internal = change.0.clone();
            Wallet::create(external, internal)
                .network(network)
                .create_wallet_no_persist()?
        }
        None => Wallet::create_single(external)
            .network(network)
            .create_wallet_no_persist()?,
    };
    let new_wallet = Arc::new(Mutex::new(created));

    let key_outer = key;
    let new_wallet_outer = new_wallet.clone();

    access_network_wallets(network, move |wallets| {
        let key_inner = key_outer.clone();
        let new_wallet_inner = new_wallet_outer.clone();

        async move {
            wallets
                .write()
                .await
                .insert((key_inner, None), new_wallet_inner);
            Ok(())
        }
    })
    .await?;

    Ok(new_wallet)
}

pub async fn get_blockchain() -> Result<EsploraAsyncClient, BitcoinWalletError> {
    debug!("Getting blockchain");
    let url = BITCOIN_EXPLORER_API.read().await.clone();
    #[cfg(not(target_arch = "wasm32"))]
    {
        EsploraBuilder::new(&url)
            .build_async()
            .map_err(|e| BitcoinWalletError::BuildEsploraError(e.to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        EsploraBuilder::new(&url)
            .build_async_with_sleeper::<WasmEsploraSleeper>()
            .map_err(|e| BitcoinWalletError::BuildEsploraError(e.to_string()))
    }
}

pub async fn sync_wallet(wallet: &MemoryWallet) -> Result<(), BitcoinWalletError> {
    let client = get_blockchain().await?;
    let mut wallet = wallet.lock().await;
    let request = wallet.start_full_scan();
    let update = client
        .full_scan(request, ESPLORA_STOP_GAP, ESPLORA_PARALLEL_REQUESTS)
        .await
        .map_err(|e| BitcoinWalletError::BdkEsploraError(*e))?;
    wallet
        .apply_update(update)
        .map_err(|e| BitcoinWalletError::BdkConnectError(e.to_string()))?;

    debug!("Wallet synced");
    Ok(())
}

async fn sync_network_wallets(wallets: NetworkWallet) -> Result<(), BitcoinWalletError> {
    for wallet in wallets.write().await.values_mut() {
        sync_wallet(wallet).await?;
    }
    Ok(())
}

pub async fn sync_wallets() -> Result<(), BitcoinWalletError> {
    let network_lock = NETWORK.read().await;
    let network = network_lock.to_owned();
    drop(network_lock);

    match network {
        Network::Bitcoin => sync_network_wallets(BDK.bitcoin.clone()).await?,
        Network::Testnet => sync_network_wallets(BDK.testnet.clone()).await?,
        Network::Testnet4 => sync_network_wallets(BDK.testnet4.clone()).await?,
        Network::Signet => sync_network_wallets(BDK.signet.clone()).await?,
        Network::Regtest => sync_network_wallets(BDK.regtest.clone()).await?,
    };

    debug!("All wallets synced");
    Ok(())
}

/// Next unused receive address. BDK 0.28 `AddressIndex::LastUnused`.
pub fn next_unused_receive_address(wallet: &mut Wallet) -> String {
    wallet
        .next_unused_address(KeychainKind::External)
        .to_string()
}

/// New receive address. BDK 0.28 `AddressIndex::New`.
pub fn reveal_next_receive_address(wallet: &mut Wallet) -> String {
    wallet
        .reveal_next_address(KeychainKind::External)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bdk_wallet::bitcoin::Network;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// Named TDD contract: construction is BDK 1.x `Wallet::create` plus
    /// `create_wallet_no_persist`. `Wallet::new(..., MemoryDatabase)` is the
    /// 0.28 API and must not compile. bdk_wallet 1.1+ names `Network::Testnet4`.
    #[test]
    fn bdk_wallet_create_no_persist_names_testnet4() {
        const EXTERNAL: &str = "wpkh(tprv8ZgxMBicQKsPdcAqYBpzAFwU5yxBUo88ggoBqu1qPcHUfSbKK1sKMLmC7EAk438btHQrSdu3jGGQa6PA71nvH5nkDexhLteJqkM4dQmWF9g/84'/1'/0'/0/*)";
        const INTERNAL: &str = "wpkh(tprv8ZgxMBicQKsPdcAqYBpzAFwU5yxBUo88ggoBqu1qPcHUfSbKK1sKMLmC7EAk438btHQrSdu3jGGQa6PA71nvH5nkDexhLteJqkM4dQmWF9g/84'/1'/0'/1/*)";

        let wallet = Wallet::create(EXTERNAL, INTERNAL)
            .network(Network::Testnet4)
            .create_wallet_no_persist()
            .expect("bdk_wallet 1.x names testnet4");
        assert_eq!(wallet.network(), Network::Testnet4);
    }
}
