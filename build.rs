use std::{collections::BTreeMap, env, fs, path};

use anyhow::{Result, anyhow};
use indexmap::map::IndexMap;
use rgbstd::stl::{
    LIB_ID_RGB_CONTRACT, LIB_ID_RGB_LOGIC as LIB_ID_RGB, LIB_ID_RGB_OPS as LIB_ID_RGB_STD,
};
use schemata::{CFA_SCHEMA_ID, NIA_SCHEMA_ID, UDA_SCHEMA_ID};
use serde::{Deserialize, Serialize};

type IdVersionMap = IndexMap<String, String>;

#[allow(non_snake_case)]
#[derive(Deserialize, Serialize)]
struct LibIds {
    LIB_ID_RGB: IdVersionMap,
    LIB_ID_RGB_CONTRACT: IdVersionMap,
    LIB_ID_RGB20: IdVersionMap,
    LIB_ID_RGB21: IdVersionMap,
    LIB_ID_RGB25: IdVersionMap,
    LIB_ID_RGB_STD: IdVersionMap,
}

const LIB_IDS_FILE: &str = "RGB_LIB_IDs.toml";
const FILE_COMMENT: &str = "# Auto-generated semantic IDs for RGB consensus-critical libraries and their corresponding versions of bitmask-core.\n\n";
const LIB_ID_RGB_COMMENT: &str =
    "[LIB_ID_RGB]\n# Consensus-breaking: If changed, assets must be reissued";
const LIB_ID_RGB_CONTRACT_COMMENT: &str = "[LIB_ID_RGB_CONTRACT]\n# Interface-only: If changed, only a new interface implementation is needed. No reiussance or migration necessary.";
const LIB_ID_RGB_STD_COMMENT: &str = "[LIB_ID_RGB_STD]\n# Not consensus-breaking: If changed, only stash and consignments must be updated. No reiussance or migration necessary.";

type HashNameMap = IndexMap<String, String>;

#[allow(non_snake_case)]
#[derive(Deserialize, Serialize)]
struct FileHashes {
    ASSETS_STOCK: HashNameMap,
    ASSETS_WALLETS: HashNameMap,
    ASSETS_TRANSFERS: HashNameMap,
    ASSETS_OFFERS: HashNameMap,
    ASSETS_BIDS: HashNameMap,
    MARKETPLACE_OFFERS: HashNameMap,
    MARKETPLACE_BIDS: HashNameMap,
}

const FILE_HASHES_FILE: &str = "file_hashes.toml";
const ASSETS_STOCK_V0: &str = "bitmask-fungible_assets_stock.c15";
const ASSETS_STOCK_V1: &str = "bitmask-assets_stock.c15";
const ASSETS_WALLETS_V0: &str = "bitmask-fungible_assets_wallets.c15";
const ASSETS_WALLETS_V1: &str = "bitmask-assets_wallets.c15";
const ASSETS_TRANSFERS: &str = "bitmask_assets_transfers.c15";
const ASSETS_OFFERS: &str = "bitmask-asset_offers.c15";
const ASSETS_BIDS: &str = "bitmask-asset_bids.c15";
const MARKETPLACE_OFFERS: &str = "bitmask-marketplace_public_offers.c15";
const MARKETPLACE_BIDS: &str = "bitmask-marketplace_public_bids.c15";
const NETWORK: &str = "bitcoin"; // Only mainnet is tracked, no monetary incentive to upgrade testnet assets

#[derive(Serialize, Deserialize, Default)]
pub struct MetricsData {
    bytes: u64,
    bytes_by_day: BTreeMap<String, u64>,
    bitcoin_wallets_by_day: BTreeMap<String, usize>,
    signet_wallets_by_day: BTreeMap<String, usize>,
    testnet_wallets_by_day: BTreeMap<String, usize>,
    regtest_wallets_by_day: BTreeMap<String, usize>,
    wallets_by_network: BTreeMap<String, usize>,
}

pub fn init_fs() -> Result<()> {
    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());
    let dir = path::Path::new(&dir);

    fs::create_dir_all(dir)?;
    fs::write(
        dir.join("metrics.json"),
        serde_json::to_string_pretty(&MetricsData::default())?,
    )?;

    Ok(())
}

fn main() -> Result<()> {
    // lib ids
    const BMC_VERSION: &str = env!("CARGO_PKG_VERSION");

    let toml = fs::read_to_string(LIB_IDS_FILE)?;
    let mut doc: LibIds = toml::from_str(&toml)?;

    doc.LIB_ID_RGB
        .entry(LIB_ID_RGB.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB_CONTRACT
        .entry(LIB_ID_RGB_CONTRACT.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB20
        .entry(NIA_SCHEMA_ID.to_string())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB21
        .entry(UDA_SCHEMA_ID.to_string())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB25
        .entry(CFA_SCHEMA_ID.to_string())
        .or_insert(BMC_VERSION.to_owned());

    doc.LIB_ID_RGB_STD
        .entry(LIB_ID_RGB_STD.to_owned())
        .or_insert(BMC_VERSION.to_owned());

    let toml = toml::to_string(&doc)?;
    let toml = toml.replace("[LIB_ID_RGB]", LIB_ID_RGB_COMMENT);
    let toml = toml.replace("[LIB_ID_RGB_CONTRACT]", LIB_ID_RGB_CONTRACT_COMMENT);
    let toml = toml.replace("[LIB_ID_RGB_STD]", LIB_ID_RGB_STD_COMMENT);

    fs::write(LIB_IDS_FILE, format!("{FILE_COMMENT}{toml}"))?;

    // file hashes
    let toml = fs::read_to_string(FILE_HASHES_FILE)?;
    let mut doc: FileHashes = toml::from_str(&toml)?;

    let assets_stock_name_v0 = format!("{LIB_ID_RGB}-{ASSETS_STOCK_V0}");
    let assets_stock_name_v1 = format!("{LIB_ID_RGB}-{ASSETS_STOCK_V1}");
    let assets_wallets_name_v0 = format!("{LIB_ID_RGB}-{ASSETS_WALLETS_V0}");
    let assets_wallets_name_v1 = format!("{LIB_ID_RGB}-{ASSETS_WALLETS_V1}");
    let assets_transfers_name = format!("{LIB_ID_RGB}-{ASSETS_TRANSFERS}");
    let assets_offers_name = format!("{LIB_ID_RGB}-{ASSETS_OFFERS}");
    let assets_bids_name = format!("{LIB_ID_RGB}-{ASSETS_BIDS}");
    let marketplace_offers_name = format!("{LIB_ID_RGB}-{MARKETPLACE_OFFERS}");
    let marketplace_bids_name = format!("{LIB_ID_RGB}-{MARKETPLACE_BIDS}");

    let assets_stock_hash_v0 = blake3::hash(assets_stock_name_v0.as_bytes())
        .to_hex()
        .to_ascii_lowercase();
    let assets_stock_hash_v1 = blake3::hash(assets_stock_name_v1.as_bytes())
        .to_hex()
        .to_ascii_lowercase();
    let assets_wallets_hash_v0 = blake3::hash(assets_wallets_name_v0.as_bytes())
        .to_hex()
        .to_ascii_lowercase();
    let assets_wallets_hash_v1 = blake3::hash(assets_wallets_name_v1.as_bytes())
        .to_hex()
        .to_ascii_lowercase();
    let assets_transfers_hash = blake3::hash(assets_transfers_name.as_bytes())
        .to_hex()
        .to_ascii_lowercase();
    let assets_offers_hash = blake3::hash(assets_offers_name.as_bytes())
        .to_hex()
        .to_ascii_lowercase();
    let assets_bids_hash = blake3::hash(assets_bids_name.as_bytes())
        .to_hex()
        .to_ascii_lowercase();
    let marketplace_offers_hash = blake3::hash(marketplace_offers_name.as_bytes())
        .to_hex()
        .to_ascii_lowercase();
    let marketplace_bids_hash = blake3::hash(marketplace_bids_name.as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    doc.ASSETS_STOCK
        .entry(format!("{NETWORK}-{assets_stock_hash_v0}.c15"))
        .or_insert(assets_stock_name_v0);

    doc.ASSETS_STOCK
        .entry(format!("{NETWORK}-{assets_stock_hash_v1}.c15"))
        .or_insert(assets_stock_name_v1);

    doc.ASSETS_WALLETS
        .entry(format!("{NETWORK}-{assets_wallets_hash_v0}.c15"))
        .or_insert(assets_wallets_name_v0);

    doc.ASSETS_WALLETS
        .entry(format!("{NETWORK}-{assets_wallets_hash_v1}.c15"))
        .or_insert(assets_wallets_name_v1);

    doc.ASSETS_TRANSFERS
        .entry(format!("{NETWORK}-{assets_transfers_hash}.c15"))
        .or_insert(assets_transfers_name);

    doc.ASSETS_OFFERS
        .entry(format!("{NETWORK}-{assets_offers_hash}.c15"))
        .or_insert(assets_offers_name);

    doc.ASSETS_BIDS
        .entry(format!("{NETWORK}-{assets_bids_hash}.c15"))
        .or_insert(assets_bids_name);

    doc.MARKETPLACE_OFFERS
        .entry(format!("{NETWORK}-{marketplace_offers_hash}.c15"))
        .or_insert(marketplace_offers_name);

    doc.MARKETPLACE_BIDS
        .entry(format!("{NETWORK}-{marketplace_bids_hash}.c15"))
        .or_insert(marketplace_bids_name);

    let toml = toml::to_string(&doc)?;

    fs::write(FILE_HASHES_FILE, toml)?;

    init_fs()?;

    built::write_built_file().map_err(|_e| anyhow!("Failed to acquire build-time information"))?;

    Ok(())
}
