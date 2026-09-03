#![allow(clippy::panic, clippy::expect_used)] // Allowed because any runtime error here will be immediately noticeable

use std::{env, str::FromStr};

use anyhow::{Result, anyhow};
use bitcoin::Network;
use once_cell::sync::Lazy;
use tokio::sync::RwLock;

use crate::error;

fn dot_env(key: &str) -> String {
    let env_file = include_str!("../.env");
    match env::var(key) {
        Ok(val) => val,
        Err(_) => {
            if let Some(line) = env_file.split('\n').find(|e| e.starts_with(key)) {
                let (_, val) = line.split_once('=').expect("value exists for key");
                val.to_owned()
            } else {
                panic!("Couldn't access .env key: {key}");
            }
        }
    }
}

pub static SERVER_WALLET_SEED: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("SERVER_WALLET_SEED")));

pub static REGTEST_DIBA_FEE_ADDRESS: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("DIBA_FEE_ADDRESS_REGTEST")));

pub static TESTNET_DIBA_FEE_ADDRESS: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("DIBA_FEE_ADDRESS_SIGNET")));

pub static SIGNET_DIBA_FEE_ADDRESS: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("DIBA_FEE_ADDRESS_SIGNET")));

pub static MAINNET_DIBA_FEE_ADDRESS: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("DIBA_FEE_ADDRESS_MAINNET")));

pub const BMC_VERSION: &str = env!("CARGO_PKG_VERSION");

pub static ELECTRUM_TIMEOUT: u8 = 4;

/// Near-term named networks. Mainnet is long-term (`SECURITY.md`).
pub const NETWORK_NAME_BITCOIN: &str = "bitcoin";
pub const NETWORK_NAME_TESTNET3: &str = "testnet3";
pub const NETWORK_NAME_TESTNET4: &str = "testnet4";
pub const NETWORK_NAME_SIGNET: &str = "signet";
pub const NETWORK_NAME_MUTINYNET: &str = "mutinynet";
pub const NETWORK_NAME_REGTEST: &str = "regtest";

/// Public Esplora HTTP explorers. Not bitcoind, electrs, or an RGB proxy.
/// [mempool.space](https://mempool.space) (accessed: 2026-08-25).
/// [mutinynet.com](https://mutinynet.com) (accessed: 2026-08-25).
pub const EXPLORER_API_TESTNET3: &str = "https://mempool.space/testnet/api";
pub const EXPLORER_API_TESTNET4: &str = "https://mempool.space/testnet4/api";
pub const EXPLORER_API_MUTINYNET: &str = "https://mutinynet.com/api";

static BITCOIN_EXPLORER_API_MAINNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_MAINNET")));
static BITCOIN_EXPLORER_API_TESTNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_TESTNET")));
static BITCOIN_EXPLORER_API_TESTNET3: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_TESTNET3")));
static BITCOIN_EXPLORER_API_TESTNET4: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_TESTNET4")));
static BITCOIN_EXPLORER_API_SIGNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_SIGNET")));
static BITCOIN_EXPLORER_API_MUTINYNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_MUTINYNET")));
static BITCOIN_EXPLORER_API_REGTEST: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_REGTEST")));
pub static BITCOIN_EXPLORER_API: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_EXPLORER_API_REGTEST")));

static BITCOIN_ELECTRUM_API_MAINNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_MAINNET")));
static BITCOIN_ELECTRUM_API_TESTNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_TESTNET")));
static BITCOIN_ELECTRUM_API_TESTNET3: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_TESTNET3")));
static BITCOIN_ELECTRUM_API_TESTNET4: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_TESTNET4")));
static BITCOIN_ELECTRUM_API_SIGNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_SIGNET")));
static BITCOIN_ELECTRUM_API_MUTINYNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_MUTINYNET")));
static BITCOIN_ELECTRUM_API_REGTEST: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_REGTEST")));
pub static BITCOIN_ELECTRUM_API: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIN_ELECTRUM_API_REGTEST")));

static BITCOIND_RPC_TESTNET3: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIND_RPC_TESTNET3")));
static BITCOIND_RPC_TESTNET4: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIND_RPC_TESTNET4")));
static BITCOIND_RPC_MUTINYNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITCOIND_RPC_MUTINYNET")));
/// Active bitcoind RPC URL for the named network. Empty until the operator fills the slot.
pub static BITCOIND_RPC: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::new()));

pub static MARKETPLACE_NOSTR: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("MARKETPLACE_NOSTR")));

pub static COORDINATOR_NOSTR: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("COORDINATOR_NOSTR")));

pub static COORDINATOR_XPUB: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("COORDINATOR_XPUB")));

pub static COORDINATOR_FEE: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("COORDINATOR_FEE")));

pub async fn get_marketplace_nostr_key() -> String {
    MARKETPLACE_NOSTR.read().await.to_string()
}

pub async fn get_coordinator_nostr_key() -> String {
    COORDINATOR_NOSTR.read().await.to_string()
}

/// Normalize an operator network name. `testnet` means testnet3.
/// Generic signet is not a product network; mutinynet is the supported signet.
/// `regtest` still parses for leftover docker env init; `switch_network` rejects it.
pub fn named_network_from_str(network_str: &str) -> Result<&'static str> {
    match network_str {
        "bitcoin" | "mainnet" => Ok(NETWORK_NAME_BITCOIN),
        "testnet" | "testnet3" => Ok(NETWORK_NAME_TESTNET3),
        "testnet4" => Ok(NETWORK_NAME_TESTNET4),
        "mutinynet" => Ok(NETWORK_NAME_MUTINYNET),
        "signet" => Err(anyhow!("generic signet is out of scope; use mutinynet")),
        "regtest" => Ok(NETWORK_NAME_REGTEST),
        other => Err(anyhow!("unknown Bitcoin network name: {other}")),
    }
}

/// Named networks `switch_network` accepts this round. Mainnet is long-term.
/// Generic signet and regtest are out of scope.
pub fn product_switch_network_supported(named: &str) -> bool {
    matches!(
        named,
        NETWORK_NAME_BITCOIN
            | NETWORK_NAME_TESTNET3
            | NETWORK_NAME_TESTNET4
            | NETWORK_NAME_MUTINYNET
    )
}

/// rust-bitcoin 0.32 `Network` for BDK. Testnet4 is a real variant.
/// Mutinynet is a custom signet; bitcoin has no Mutinynet variant.
pub fn bitcoin_network_from_named(named: &str) -> Result<Network> {
    match named {
        NETWORK_NAME_BITCOIN => Ok(Network::Bitcoin),
        NETWORK_NAME_TESTNET3 => Ok(Network::Testnet),
        NETWORK_NAME_TESTNET4 => Ok(Network::Testnet4),
        NETWORK_NAME_SIGNET | NETWORK_NAME_MUTINYNET => Ok(Network::Signet),
        NETWORK_NAME_REGTEST => Ok(Network::Regtest),
        other => Network::from_str(other).map_err(|e| anyhow!("{e}")),
    }
}

fn bitcoin_network_for_named(named: &str) -> Result<Network> {
    bitcoin_network_from_named(named)
}

/// Parse the product named-network string as `bpwallet::Network`.
/// Mutinynet is a custom signet; bp-wallet 0.11 has no Mutinynet variant.
pub fn bp_network_from_str(
    s: &str,
) -> Result<bpwallet::Network, <bpwallet::Network as FromStr>::Err> {
    match named_network_from_str(s) {
        Ok(NETWORK_NAME_BITCOIN) => Ok(bpwallet::Network::Mainnet),
        Ok(NETWORK_NAME_TESTNET3) => Ok(bpwallet::Network::Testnet3),
        Ok(NETWORK_NAME_TESTNET4) => Ok(bpwallet::Network::Testnet4),
        Ok(NETWORK_NAME_SIGNET) | Ok(NETWORK_NAME_MUTINYNET) => Ok(bpwallet::Network::Signet),
        Ok(NETWORK_NAME_REGTEST) => Ok(bpwallet::Network::Regtest),
        Ok(_) | Err(_) => bpwallet::Network::from_str(s),
    }
}

pub async fn get_diba_fee() -> String {
    let named = get_network().await;
    let network = bitcoin_network_from_named(&named).expect("network should be valid");
    match network {
        Network::Bitcoin => MAINNET_DIBA_FEE_ADDRESS.read().await.to_string(),
        Network::Testnet | Network::Testnet4 => TESTNET_DIBA_FEE_ADDRESS.read().await.to_string(),
        Network::Signet => SIGNET_DIBA_FEE_ADDRESS.read().await.to_string(),
        Network::Regtest => REGTEST_DIBA_FEE_ADDRESS.read().await.to_string(),
    }
}
pub async fn get_server_wallet_seed() -> String {
    SERVER_WALLET_SEED.read().await.to_string()
}

// Descriptor strings
pub const BTC_MAINNET_TAP_PATH: &str = "m/86h/0h/0h";
pub const BTC_TESTNET_TAP_PATH: &str = "m/86h/1h/0h";
pub const BTC_MAINNET_WPKH_PATH: &str = "m/84h/0h/0h";

pub const BTC_TESTNET_WPKH_PATH: &str = "m/84h/1h/0h";

/// Default is P2TR (BIP-86). Bitcoin and RGB share this account path.
pub async fn get_btc_path() -> String {
    let named = get_network().await;
    let network = bitcoin_network_from_named(&named).expect("network should be valid");
    match network {
        Network::Bitcoin => BTC_MAINNET_TAP_PATH.to_owned(),
        _ => BTC_TESTNET_TAP_PATH.to_owned(),
    }
}

pub async fn get_rgb_path() -> String {
    get_btc_path().await
}

pub async fn bad_btc_path() -> String {
    BTC_TESTNET_WPKH_PATH.to_owned()
}

pub async fn bad_rgb_path() -> String {
    BTC_TESTNET_TAP_PATH.to_owned()
}

// Magic number for versioning descriptors
pub const DIBA_DESCRIPTOR_VERSION: u8 = 0;
pub const DIBA_MAGIC_NO: [u8; 4] = *b"DIBA";
pub const DIBA_DESCRIPTOR: [u8; 5] = [
    DIBA_MAGIC_NO[0],
    DIBA_MAGIC_NO[1],
    DIBA_MAGIC_NO[2],
    DIBA_MAGIC_NO[3],
    DIBA_DESCRIPTOR_VERSION,
];

pub static NETWORK: Lazy<RwLock<Network>> = Lazy::new(|| {
    let named = named_network_from_str(&dot_env("BITCOIN_NETWORK")).expect("Parse Bitcoin network");
    RwLock::new(bitcoin_network_for_named(named).expect("Parse Bitcoin network"))
});

pub static NAMED_NETWORK: Lazy<RwLock<String>> = Lazy::new(|| {
    let named = named_network_from_str(&dot_env("BITCOIN_NETWORK")).expect("Parse Bitcoin network");
    RwLock::new(named.to_owned())
});

/// Product network name RGB, BP, Carbonado filenames, wasm, and bitmaskd parse.
/// This is `testnet4` / `mutinynet`, not rust-bitcoin Display (`testnet` / `signet`).
pub async fn get_network() -> String {
    NAMED_NETWORK.read().await.to_string()
}

pub async fn get_named_network() -> String {
    NAMED_NETWORK.read().await.to_string()
}

/// Switch Bitcoin network.
/// Accepts bitcoin (long-term), testnet/testnet3, testnet4, and mutinynet.
/// Generic signet and regtest are out of scope.
/// Copies that named network's bitcoind, Electrum, Esplora, and RGB-proxy slots.
/// Slot strings stay empty until the operator fills them.
pub async fn switch_network(network_str: &str) -> Result<()> {
    let named = named_network_from_str(network_str)?;
    if !product_switch_network_supported(named) {
        return Err(anyhow!(
            "{named} is out of scope; supported named networks are testnet3, testnet4, and mutinynet"
        ));
    }
    let network = bitcoin_network_for_named(named)?;

    let (bitcoind, electrum, esplora, proxy) = match named {
        NETWORK_NAME_BITCOIN => (
            String::new(),
            BITCOIN_ELECTRUM_API_MAINNET.read().await.to_owned(),
            BITCOIN_EXPLORER_API_MAINNET.read().await.to_owned(),
            String::new(),
        ),
        NETWORK_NAME_TESTNET3 => (
            BITCOIND_RPC_TESTNET3.read().await.to_owned(),
            BITCOIN_ELECTRUM_API_TESTNET3.read().await.to_owned(),
            BITCOIN_EXPLORER_API_TESTNET3.read().await.to_owned(),
            RGB_PROXY_ENDPOINT_TESTNET3.read().await.to_owned(),
        ),
        NETWORK_NAME_TESTNET4 => (
            BITCOIND_RPC_TESTNET4.read().await.to_owned(),
            BITCOIN_ELECTRUM_API_TESTNET4.read().await.to_owned(),
            BITCOIN_EXPLORER_API_TESTNET4.read().await.to_owned(),
            RGB_PROXY_ENDPOINT_TESTNET4.read().await.to_owned(),
        ),
        NETWORK_NAME_MUTINYNET => (
            BITCOIND_RPC_MUTINYNET.read().await.to_owned(),
            BITCOIN_ELECTRUM_API_MUTINYNET.read().await.to_owned(),
            BITCOIN_EXPLORER_API_MUTINYNET.read().await.to_owned(),
            RGB_PROXY_ENDPOINT_MUTINYNET.read().await.to_owned(),
        ),
        other => {
            return Err(anyhow!(
                "{other} is out of scope; supported named networks are testnet3, testnet4, and mutinynet"
            ));
        }
    };

    *BITCOIND_RPC.write().await = bitcoind;
    *BITCOIN_ELECTRUM_API.write().await = electrum;
    *BITCOIN_EXPLORER_API.write().await = esplora;
    *RGB_PROXY_ENDPOINT.write().await = proxy;
    *NETWORK.write().await = network;
    *NAMED_NETWORK.write().await = named.to_owned();

    Ok(())
}

// lightning
pub static LNDHUB_ENDPOINT: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("LNDHUB_ENDPOINT")));

// bitmask node
pub static BITMASK_ENDPOINT: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("BITMASK_ENDPOINT")));

// rgb proxy node. Named-network slots stay empty until the operator fills them.
static RGB_PROXY_ENDPOINT_TESTNET3: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("RGB_PROXY_ENDPOINT_TESTNET3")));
static RGB_PROXY_ENDPOINT_TESTNET4: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("RGB_PROXY_ENDPOINT_TESTNET4")));
static RGB_PROXY_ENDPOINT_MUTINYNET: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("RGB_PROXY_ENDPOINT_MUTINYNET")));
pub static RGB_PROXY_ENDPOINT: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("RGB_PROXY_ENDPOINT")));

// carbonado
pub static CARBONADO_ENDPOINT: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new(dot_env("CARBONADO_ENDPOINT")));

pub async fn get_env(key: &str) -> String {
    match key {
        "LNDHUB_ENDPOINT" => LNDHUB_ENDPOINT.read().await.to_string(),
        "BITMASK_ENDPOINT" => BITMASK_ENDPOINT.read().await.to_string(),
        "CARBONADO_ENDPOINT" => CARBONADO_ENDPOINT.read().await.to_string(),
        "BITCOIN_EXPLORER_API_MAINNET" => BITCOIN_EXPLORER_API_MAINNET.read().await.to_string(),
        "BITCOIN_EXPLORER_API_TESTNET" => BITCOIN_EXPLORER_API_TESTNET.read().await.to_string(),
        "BITCOIN_EXPLORER_API_TESTNET3" => BITCOIN_EXPLORER_API_TESTNET3.read().await.to_string(),
        "BITCOIN_EXPLORER_API_TESTNET4" => BITCOIN_EXPLORER_API_TESTNET4.read().await.to_string(),
        "BITCOIN_EXPLORER_API_SIGNET" => BITCOIN_EXPLORER_API_SIGNET.read().await.to_string(),
        "BITCOIN_EXPLORER_API_MUTINYNET" => BITCOIN_EXPLORER_API_MUTINYNET.read().await.to_string(),
        "BITCOIN_EXPLORER_API_REGTEST" => BITCOIN_EXPLORER_API_REGTEST.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_MAINNET" => BITCOIN_ELECTRUM_API_MAINNET.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_TESTNET" => BITCOIN_ELECTRUM_API_TESTNET.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_TESTNET3" => BITCOIN_ELECTRUM_API_TESTNET3.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_TESTNET4" => BITCOIN_ELECTRUM_API_TESTNET4.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_SIGNET" => BITCOIN_ELECTRUM_API_SIGNET.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_MUTINYNET" => BITCOIN_ELECTRUM_API_MUTINYNET.read().await.to_string(),
        "BITCOIN_ELECTRUM_API_REGTEST" => BITCOIN_ELECTRUM_API_REGTEST.read().await.to_string(),
        "BITCOIND_RPC_TESTNET3" => BITCOIND_RPC_TESTNET3.read().await.to_string(),
        "BITCOIND_RPC_TESTNET4" => BITCOIND_RPC_TESTNET4.read().await.to_string(),
        "BITCOIND_RPC_MUTINYNET" => BITCOIND_RPC_MUTINYNET.read().await.to_string(),
        "RGB_PROXY_ENDPOINT" => RGB_PROXY_ENDPOINT.read().await.to_string(),
        "RGB_PROXY_ENDPOINT_TESTNET3" => RGB_PROXY_ENDPOINT_TESTNET3.read().await.to_string(),
        "RGB_PROXY_ENDPOINT_TESTNET4" => RGB_PROXY_ENDPOINT_TESTNET4.read().await.to_string(),
        "RGB_PROXY_ENDPOINT_MUTINYNET" => RGB_PROXY_ENDPOINT_MUTINYNET.read().await.to_string(),
        _ => {
            error!(format!("get_env called an unknown key, {key}"));
            "".to_owned()
        }
    }
}

pub async fn set_env(key: &str, value: &str) {
    match key {
        "LNDHUB_ENDPOINT" => value.clone_into(&mut (*LNDHUB_ENDPOINT.write().await)),
        "BITMASK_ENDPOINT" => value.clone_into(&mut (*BITMASK_ENDPOINT.write().await)),
        "CARBONADO_ENDPOINT" => value.clone_into(&mut (*CARBONADO_ENDPOINT.write().await)),
        "BITCOIN_EXPLORER_API_MAINNET" => {
            value.clone_into(&mut (*BITCOIN_EXPLORER_API_MAINNET.write().await))
        }
        "BITCOIN_EXPLORER_API_TESTNET" => {
            value.clone_into(&mut (*BITCOIN_EXPLORER_API_TESTNET.write().await))
        }
        "BITCOIN_EXPLORER_API_TESTNET3" => {
            value.clone_into(&mut (*BITCOIN_EXPLORER_API_TESTNET3.write().await))
        }
        "BITCOIN_EXPLORER_API_TESTNET4" => {
            value.clone_into(&mut (*BITCOIN_EXPLORER_API_TESTNET4.write().await))
        }
        "BITCOIN_EXPLORER_API_SIGNET" => {
            value.clone_into(&mut (*BITCOIN_EXPLORER_API_SIGNET.write().await))
        }
        "BITCOIN_EXPLORER_API_MUTINYNET" => {
            value.clone_into(&mut (*BITCOIN_EXPLORER_API_MUTINYNET.write().await))
        }
        "BITCOIN_EXPLORER_API_REGTEST" => {
            value.clone_into(&mut (*BITCOIN_EXPLORER_API_REGTEST.write().await))
        }
        "BITCOIN_ELECTRUM_API_MAINNET" => {
            value.clone_into(&mut (*BITCOIN_ELECTRUM_API_MAINNET.write().await))
        }
        "BITCOIN_ELECTRUM_API_TESTNET" => {
            value.clone_into(&mut (*BITCOIN_ELECTRUM_API_TESTNET.write().await))
        }
        "BITCOIN_ELECTRUM_API_TESTNET3" => {
            value.clone_into(&mut (*BITCOIN_ELECTRUM_API_TESTNET3.write().await))
        }
        "BITCOIN_ELECTRUM_API_TESTNET4" => {
            value.clone_into(&mut (*BITCOIN_ELECTRUM_API_TESTNET4.write().await))
        }
        "BITCOIN_ELECTRUM_API_SIGNET" => {
            value.clone_into(&mut (*BITCOIN_ELECTRUM_API_SIGNET.write().await))
        }
        "BITCOIN_ELECTRUM_API_MUTINYNET" => {
            value.clone_into(&mut (*BITCOIN_ELECTRUM_API_MUTINYNET.write().await))
        }
        "BITCOIN_ELECTRUM_API_REGTEST" => {
            value.clone_into(&mut (*BITCOIN_ELECTRUM_API_REGTEST.write().await))
        }
        "BITCOIND_RPC_TESTNET3" => value.clone_into(&mut (*BITCOIND_RPC_TESTNET3.write().await)),
        "BITCOIND_RPC_TESTNET4" => value.clone_into(&mut (*BITCOIND_RPC_TESTNET4.write().await)),
        "BITCOIND_RPC_MUTINYNET" => value.clone_into(&mut (*BITCOIND_RPC_MUTINYNET.write().await)),
        "RGB_PROXY_ENDPOINT" => value.clone_into(&mut (*RGB_PROXY_ENDPOINT.write().await)),
        "RGB_PROXY_ENDPOINT_TESTNET3" => {
            value.clone_into(&mut (*RGB_PROXY_ENDPOINT_TESTNET3.write().await))
        }
        "RGB_PROXY_ENDPOINT_TESTNET4" => {
            value.clone_into(&mut (*RGB_PROXY_ENDPOINT_TESTNET4.write().await))
        }
        "RGB_PROXY_ENDPOINT_MUTINYNET" => {
            value.clone_into(&mut (*RGB_PROXY_ENDPOINT_MUTINYNET.write().await))
        }
        _ => {
            error!(format!("set_env called an unknown key, {key}"));
        }
    };
}

pub const FEE_ADDRESS: &str = "bc1puswjfgvrmd37yca7h39m045ym40wgdajjdp9hazllxdez927nu2qpfuj0j";

pub mod storage_keys {
    pub const ACCOUNT: &str = "bitmask-account";
    pub const WALLET_DESCR: &str = "bitmask-wallet-desc";
    pub const WALLET_CACHE: &str = "bitmask-wallet-cache";
    pub const WALLET_DATA: &str = "bitmask-wallet-data";
    pub const WALLET_L2: &str = "bitmask-wallet-l2";

    pub const RGB_STOCK: &str = "bitmask-rgb_stock";
    pub const RGB_WALLET: &str = "bitmask-rgb_wallet";
    pub const RGB_STOCK_STASH: &str = "bitmask-rgb_stock_stash";
    pub const RGB_STOCK_STATE: &str = "bitmask-rgb_stock_state";
    pub const RGB_STOCK_INDEX: &str = "bitmask-rgb_stock_index";
}

pub mod deprecated_storage_keys {
    pub const ASSETS_STOCK: &str = "bitmask-assets_stock.c15";
    pub const ASSETS_WALLETS: &str = "bitmask-assets_wallets.c15";
    pub const ASSETS_TRANSFERS: &str = "bitmask_assets_transfers.c15";
    pub const ASSETS_OFFERS: &str = "bitmask-asset_offers.c15";
    pub const ASSETS_BIDS: &str = "bitmask-asset_bids.c15";
    pub const MARKETPLACE_OFFERS: &str = "bitmask-marketplace_public_offers.c15";
    pub const BITCOIN_WALLETS: &str = "bitmask-bitcoin_wallets.c15";
    pub const OLD_ASSETS_STOCK: &str = "bitmask-fungible_assets_stock.c15";
    pub const OLD_ASSETS_WALLETS: &str = "bitmask-fungible_assets_wallets.c15";
}
