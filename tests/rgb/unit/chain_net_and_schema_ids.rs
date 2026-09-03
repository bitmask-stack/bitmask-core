#![cfg(not(target_arch = "wasm32"))]

use bpwallet::Network;
use rgbcore::ChainNet;
use schemata::{CFA_SCHEMA_ID, IFA_SCHEMA_ID, NIA_SCHEMA_ID, PFA_SCHEMA_ID, UDA_SCHEMA_ID};

use bitmask_core::rgb::models::AssetType;

#[test]
fn rgb_0_11_chain_net_and_schema_ids() {
    assert_eq!(Network::Testnet3.to_string(), "testnet3");
    assert_eq!(Network::Testnet4.to_string(), "testnet4");
    assert_eq!(
        ChainNet::BitcoinTestnet4,
        ChainNet::BitcoinTestnet4,
        "RGB 0.11 ChainNet names BitcoinTestnet4"
    );
    assert!(
        !NIA_SCHEMA_ID.to_string().is_empty(),
        "NIA schema id is present"
    );
    assert!(
        !UDA_SCHEMA_ID.to_string().is_empty(),
        "UDA schema id is present"
    );
}

#[test]
fn asset_type_maps_cfa_ifa_pfa_not_rgb25() {
    assert_eq!(
        AssetType::from_schema(NIA_SCHEMA_ID),
        Some(AssetType::RGB20)
    );
    assert_eq!(
        AssetType::from_schema(UDA_SCHEMA_ID),
        Some(AssetType::RGB21)
    );
    assert_eq!(AssetType::from_schema(CFA_SCHEMA_ID), Some(AssetType::CFA));
    assert_eq!(AssetType::from_schema(IFA_SCHEMA_ID), Some(AssetType::IFA));
    assert_eq!(AssetType::from_schema(PFA_SCHEMA_ID), Some(AssetType::PFA));
    assert_eq!(AssetType::CFA.to_interface_name(), "CFA");
    assert_eq!(AssetType::IFA.to_interface_name(), "IFA");
    assert_eq!(AssetType::PFA.to_interface_name(), "PFA");
}
