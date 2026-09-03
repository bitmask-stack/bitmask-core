#![cfg(not(target_arch = "wasm32"))]

use bitmask_core::constants::{
    NETWORK, NETWORK_NAME_MUTINYNET, NETWORK_NAME_TESTNET4, get_network, switch_network,
};
use bitmask_core::proxy::{handle_file, proxy_prefixed_name};

#[test]
fn proxy_prefix_uses_named_network_not_bitcoin_display() {
    assert_eq!(
        proxy_prefixed_name("consignment.bin", NETWORK_NAME_TESTNET4),
        "testnet4-consignment.bin"
    );
    assert_eq!(
        proxy_prefixed_name("consignment.bin", NETWORK_NAME_MUTINYNET),
        "mutinynet-consignment.bin"
    );
    assert_ne!(
        proxy_prefixed_name("consignment.bin", NETWORK_NAME_TESTNET4),
        "testnet-consignment.bin",
        "must not collide with bitcoin 0.29 testnet3 Display"
    );
    assert_ne!(
        proxy_prefixed_name("consignment.bin", NETWORK_NAME_MUTINYNET),
        "signet-consignment.bin",
        "must not collide with bitcoin 0.29 signet Display"
    );
}

#[tokio::test]
async fn switch_testnet4_proxy_prefix_is_named_not_bitcoin_display() {
    switch_network("testnet4")
        .await
        .expect("switch to testnet4");
    assert_eq!(get_network().await, NETWORK_NAME_TESTNET4);
    assert_eq!(
        NETWORK.read().await.to_string(),
        NETWORK_NAME_TESTNET4,
        "bitcoin 0.32 Display of Network::Testnet4 is testnet4, not 0.29 testnet"
    );

    let path = handle_file("consignment.bin", 0)
        .await
        .expect("proxy handle_file");
    let fname = path
        .file_name()
        .expect("filename")
        .to_string_lossy()
        .into_owned();
    assert_eq!(fname, "testnet4-consignment.bin");
    assert!(!fname.starts_with("testnet-c"), "{fname}");

    switch_network("mutinynet")
        .await
        .expect("switch to mutinynet");
    let path = handle_file("consignment.bin", 0)
        .await
        .expect("proxy handle_file mutinynet");
    let fname = path
        .file_name()
        .expect("filename")
        .to_string_lossy()
        .into_owned();
    assert_eq!(fname, "mutinynet-consignment.bin");
    assert!(!fname.starts_with("signet-"), "{fname}");
}
