#![cfg(not(target_arch = "wasm32"))]

use std::str::FromStr;

use bitcoin::Network;
use bitmask_core::constants::{
    BITCOIN_ELECTRUM_API, BITCOIN_EXPLORER_API, BITCOIND_RPC, EXPLORER_API_MUTINYNET,
    EXPLORER_API_TESTNET3, EXPLORER_API_TESTNET4, NETWORK_NAME_MUTINYNET, NETWORK_NAME_TESTNET3,
    NETWORK_NAME_TESTNET4, RGB_PROXY_ENDPOINT, bitcoin_network_from_named, bp_network_from_str,
    get_env, get_named_network, get_network, named_network_from_str,
    product_switch_network_supported, set_env, switch_network,
};

fn env_value<'a>(env: &'a str, key: &str) -> &'a str {
    let prefix = format!("{key}=");
    env.lines()
        .find(|line| line.starts_with(&prefix))
        .and_then(|line| line.split_once('='))
        .map(|(_, val)| val)
        .unwrap_or_else(|| panic!("missing .env key {key}"))
}

#[test]
fn named_network_aliases() {
    assert_eq!(
        named_network_from_str("testnet").expect("testnet"),
        NETWORK_NAME_TESTNET3
    );
    assert_eq!(
        named_network_from_str("testnet3").expect("testnet3"),
        NETWORK_NAME_TESTNET3
    );
    assert_eq!(
        named_network_from_str("testnet4").expect("testnet4"),
        NETWORK_NAME_TESTNET4
    );
    assert_eq!(
        named_network_from_str("mutinynet").expect("mutinynet"),
        NETWORK_NAME_MUTINYNET
    );
    assert!(named_network_from_str("not-a-network").is_err());
}

/// Named product networks map onto bitcoin 0.32 chain names.
/// testnet3 -> `testnet`, testnet4 -> `testnet4`, mutinynet -> `signet`
/// (mutinynet is a signet; generic signet is not a product network).
#[test]
fn named_networks_map_to_bitcoin_chain() {
    assert_eq!(
        bitcoin_network_from_named(NETWORK_NAME_TESTNET3).expect("testnet3"),
        Network::Testnet
    );
    assert_eq!(
        bitcoin_network_from_named(NETWORK_NAME_TESTNET4).expect("testnet4"),
        Network::Testnet4
    );
    assert_eq!(
        bitcoin_network_from_named(NETWORK_NAME_MUTINYNET).expect("mutinynet"),
        Network::Signet
    );
    assert_eq!(Network::Testnet.to_string(), "testnet");
    assert_eq!(Network::Testnet4.to_string(), "testnet4");
    assert_eq!(Network::Signet.to_string(), "signet");
    assert_ne!(
        NETWORK_NAME_TESTNET3,
        Network::Testnet.to_string(),
        "RGB and Carbonado keep testnet3, not bitcoin Display testnet"
    );
    assert_eq!(NETWORK_NAME_TESTNET4, Network::Testnet4.to_string());
    assert_ne!(
        NETWORK_NAME_MUTINYNET,
        Network::Signet.to_string(),
        "RGB and Carbonado keep mutinynet, not bitcoin Display signet"
    );
}

/// wasm `lib/web/constants.ts` Network names must match product switch names.
/// Generic signet and regtest are not switchable. `testnet` aliases testnet3.
#[test]
fn wasm_helper_network_enum_uses_product_names() {
    let ts = include_str!("../../../lib/web/constants.ts");
    for name in ["bitcoin", "testnet3", "testnet4", "mutinynet"] {
        assert!(
            product_switch_network_supported(name),
            "{name} is a product switch name"
        );
        assert!(
            ts.contains(&format!("{name} = \"{name}\"")),
            "{name} must be a wasm helper Network name"
        );
    }
    assert!(!product_switch_network_supported("signet"));
    assert!(!product_switch_network_supported("regtest"));
    assert!(
        !ts.contains("signet = \"signet\""),
        "generic signet is not a wasm helper switch name"
    );
    assert!(
        !ts.contains("regtest = \"regtest\""),
        "generic regtest is not a wasm helper switch name"
    );
    assert!(
        ts.contains("testnet = \"testnet3\""),
        "stored testnet aliases testnet3"
    );
    assert!(
        ts.contains("networkFromStored"),
        "init maps stored names; signet and regtest are not switchable"
    );
}

#[test]
fn named_network_from_str_rejects_generic_signet() {
    let err = named_network_from_str("signet").expect_err("generic signet is out of scope");
    let msg = err.to_string();
    assert!(
        msg.contains("signet") || msg.contains("mutinynet") || msg.contains("out of scope"),
        "{msg}"
    );
}

/// Mutinynet is a custom signet. bp-wallet 0.11 has no Mutinynet variant.
#[test]
fn mutinynet_maps_to_bpwallet_signet() {
    let bp = bp_network_from_str(NETWORK_NAME_MUTINYNET).expect("mutinynet is a signet");
    assert_eq!(bp, bpwallet::Network::Signet);
}

/// bitcoind, Electrum, and RGB-proxy slots for the three named networks stay
/// empty in `.env` until the operator fills them. Public Esplora constants
/// are named separately and are not private node URLs.
#[test]
fn env_named_network_daemon_slots_are_empty() {
    let env = include_str!("../../../.env");
    for key in [
        "BITCOIND_RPC_TESTNET3",
        "BITCOIND_RPC_TESTNET4",
        "BITCOIND_RPC_MUTINYNET",
        "BITCOIN_ELECTRUM_API_TESTNET3",
        "BITCOIN_ELECTRUM_API_TESTNET4",
        "BITCOIN_ELECTRUM_API_MUTINYNET",
        "RGB_PROXY_ENDPOINT_TESTNET3",
        "RGB_PROXY_ENDPOINT_TESTNET4",
        "RGB_PROXY_ENDPOINT_MUTINYNET",
    ] {
        assert_eq!(
            env_value(env, key),
            "",
            "{key} must stay empty until the operator fills it"
        );
    }
}

#[tokio::test]
async fn switch_network_rejects_generic_signet_and_regtest() {
    let signet = switch_network("signet")
        .await
        .expect_err("generic signet is out of scope");
    let signet_msg = signet.to_string();
    assert!(
        signet_msg.contains("signet")
            || signet_msg.contains("out of scope")
            || signet_msg.contains("mutinynet"),
        "{signet_msg}"
    );

    let regtest = switch_network("regtest")
        .await
        .expect_err("regtest is out of scope");
    let regtest_msg = regtest.to_string();
    assert!(
        regtest_msg.contains("regtest") || regtest_msg.contains("out of scope"),
        "{regtest_msg}"
    );
}

#[tokio::test]
async fn switch_network_slots_do_not_share_urls() {
    set_env("BITCOIND_RPC_TESTNET3", "http://t3-bitcoind.invalid").await;
    set_env("BITCOIND_RPC_TESTNET4", "http://t4-bitcoind.invalid").await;
    set_env("BITCOIND_RPC_MUTINYNET", "http://mutiny-bitcoind.invalid").await;
    set_env("BITCOIN_ELECTRUM_API_TESTNET3", "t3-electrum.invalid:50001").await;
    set_env("BITCOIN_ELECTRUM_API_TESTNET4", "t4-electrum.invalid:50001").await;
    set_env(
        "BITCOIN_ELECTRUM_API_MUTINYNET",
        "mutiny-electrum.invalid:50001",
    )
    .await;
    set_env("BITCOIN_EXPLORER_API_TESTNET3", "http://t3-esplora.invalid").await;
    set_env("BITCOIN_EXPLORER_API_TESTNET4", "http://t4-esplora.invalid").await;
    set_env(
        "BITCOIN_EXPLORER_API_MUTINYNET",
        "http://mutiny-esplora.invalid",
    )
    .await;
    set_env("RGB_PROXY_ENDPOINT_TESTNET3", "http://t3-rgb-proxy.invalid").await;
    set_env("RGB_PROXY_ENDPOINT_TESTNET4", "http://t4-rgb-proxy.invalid").await;
    set_env(
        "RGB_PROXY_ENDPOINT_MUTINYNET",
        "http://mutiny-rgb-proxy.invalid",
    )
    .await;

    switch_network("testnet4")
        .await
        .expect("switch to testnet4");
    assert_eq!(get_named_network().await, NETWORK_NAME_TESTNET4);
    assert_eq!(
        BITCOIND_RPC.read().await.as_str(),
        "http://t4-bitcoind.invalid"
    );
    assert_eq!(
        BITCOIN_ELECTRUM_API.read().await.as_str(),
        "t4-electrum.invalid:50001"
    );
    assert_eq!(
        BITCOIN_EXPLORER_API.read().await.as_str(),
        "http://t4-esplora.invalid"
    );
    assert_eq!(
        RGB_PROXY_ENDPOINT.read().await.as_str(),
        "http://t4-rgb-proxy.invalid"
    );
    assert_ne!(
        BITCOIND_RPC.read().await.as_str(),
        "http://t3-bitcoind.invalid"
    );
    assert_ne!(
        BITCOIND_RPC.read().await.as_str(),
        "http://mutiny-bitcoind.invalid"
    );
    assert_eq!(
        get_env("BITCOIND_RPC_TESTNET3").await,
        "http://t3-bitcoind.invalid"
    );
    assert_eq!(
        get_env("BITCOIN_ELECTRUM_API_TESTNET3").await,
        "t3-electrum.invalid:50001"
    );
    assert_eq!(
        get_env("RGB_PROXY_ENDPOINT_MUTINYNET").await,
        "http://mutiny-rgb-proxy.invalid"
    );

    switch_network("testnet3")
        .await
        .expect("switch to testnet3");
    assert_eq!(
        BITCOIND_RPC.read().await.as_str(),
        "http://t3-bitcoind.invalid"
    );
    assert_eq!(
        BITCOIN_ELECTRUM_API.read().await.as_str(),
        "t3-electrum.invalid:50001"
    );
    assert_eq!(
        BITCOIN_EXPLORER_API.read().await.as_str(),
        "http://t3-esplora.invalid"
    );
    assert_eq!(
        RGB_PROXY_ENDPOINT.read().await.as_str(),
        "http://t3-rgb-proxy.invalid"
    );
    assert_ne!(
        BITCOIN_ELECTRUM_API.read().await.as_str(),
        "t4-electrum.invalid:50001"
    );

    switch_network("mutinynet")
        .await
        .expect("switch to mutinynet");
    assert_eq!(
        BITCOIND_RPC.read().await.as_str(),
        "http://mutiny-bitcoind.invalid"
    );
    assert_eq!(
        BITCOIN_ELECTRUM_API.read().await.as_str(),
        "mutiny-electrum.invalid:50001"
    );
    assert_eq!(
        BITCOIN_EXPLORER_API.read().await.as_str(),
        "http://mutiny-esplora.invalid"
    );
    assert_eq!(
        RGB_PROXY_ENDPOINT.read().await.as_str(),
        "http://mutiny-rgb-proxy.invalid"
    );
    assert_ne!(
        RGB_PROXY_ENDPOINT.read().await.as_str(),
        "http://t4-rgb-proxy.invalid"
    );

    set_env("BITCOIND_RPC_TESTNET3", "").await;
    set_env("BITCOIND_RPC_TESTNET4", "").await;
    set_env("BITCOIND_RPC_MUTINYNET", "").await;
    set_env("BITCOIN_ELECTRUM_API_TESTNET3", "").await;
    set_env("BITCOIN_ELECTRUM_API_TESTNET4", "").await;
    set_env("BITCOIN_ELECTRUM_API_MUTINYNET", "").await;
    set_env("BITCOIN_EXPLORER_API_TESTNET3", EXPLORER_API_TESTNET3).await;
    set_env("BITCOIN_EXPLORER_API_TESTNET4", EXPLORER_API_TESTNET4).await;
    set_env("BITCOIN_EXPLORER_API_MUTINYNET", EXPLORER_API_MUTINYNET).await;
    set_env("RGB_PROXY_ENDPOINT_TESTNET3", "").await;
    set_env("RGB_PROXY_ENDPOINT_TESTNET4", "").await;
    set_env("RGB_PROXY_ENDPOINT_MUTINYNET", "").await;
}

#[tokio::test]
async fn switch_testnet4_is_bpwallet_testnet4_after_get_network_roundtrip() {
    set_env("BITCOIN_EXPLORER_API_TESTNET4", EXPLORER_API_TESTNET4).await;
    switch_network("testnet4")
        .await
        .expect("switch to testnet4");
    let named = get_network().await;
    assert_eq!(named, NETWORK_NAME_TESTNET4);
    let bp = bpwallet::Network::from_str(&named).expect("RGB parses get_network()");
    assert_eq!(bp, bpwallet::Network::Testnet4);
    assert_eq!(
        bp_network_from_str(&named).expect("product parser"),
        bpwallet::Network::Testnet4
    );
    switch_network(&get_network().await)
        .await
        .expect("bitmaskd startup round-trip");
    assert_eq!(get_named_network().await, NETWORK_NAME_TESTNET4);
    assert_eq!(get_network().await, NETWORK_NAME_TESTNET4);
    assert_eq!(
        BITCOIN_EXPLORER_API.read().await.as_str(),
        EXPLORER_API_TESTNET4
    );
    bitmask_core::bitcoin::get_blockchain()
        .await
        .expect("BDK 1.x names testnet4; esplora URL is the public testnet4 explorer");
}

#[tokio::test]
async fn switch_mutinynet_survives_get_network_roundtrip() {
    set_env("BITCOIN_EXPLORER_API_MUTINYNET", EXPLORER_API_MUTINYNET).await;
    switch_network("mutinynet")
        .await
        .expect("switch to mutinynet");
    let named = get_network().await;
    assert_eq!(named, NETWORK_NAME_MUTINYNET);
    assert_eq!(
        bp_network_from_str(&named).expect("product parser"),
        bpwallet::Network::Signet
    );
    switch_network(&get_network().await)
        .await
        .expect("bitmaskd startup round-trip");
    assert_eq!(get_named_network().await, NETWORK_NAME_MUTINYNET);
    assert_eq!(get_network().await, NETWORK_NAME_MUTINYNET);
    assert_eq!(
        BITCOIN_EXPLORER_API.read().await.as_str(),
        EXPLORER_API_MUTINYNET
    );
}

#[tokio::test]
async fn switch_network_sets_public_explorers() {
    set_env("BITCOIN_EXPLORER_API_TESTNET3", EXPLORER_API_TESTNET3).await;
    switch_network("testnet3")
        .await
        .expect("switch to testnet3");
    assert_eq!(get_named_network().await, NETWORK_NAME_TESTNET3);
    assert_eq!(get_network().await, NETWORK_NAME_TESTNET3);
    assert_eq!(
        BITCOIN_EXPLORER_API.read().await.as_str(),
        EXPLORER_API_TESTNET3
    );
}
