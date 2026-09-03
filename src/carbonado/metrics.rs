#![cfg(feature = "server")]
#![cfg(not(target_arch = "wasm32"))]
use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use heed3::types::Bytes;
use heed3::{Database, Env, EnvOpenOptions};
use log::{debug, error};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::fs;
use walkdir::WalkDir;

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

const MAINNET_WALLET_V010: &str =
    "bitcoin-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15";
const TESTNET_WALLET_V010: &str =
    "testnet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15";
const SIGNET_WALLET_V010: &str =
    "signet-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15";
const REGTEST_WALLET_V010: &str =
    "regtest-6075e9716c984b37840f76ad2b50b3d1b98ed286884e5ceba5bcc8e6b74988d3.c15";
const RGB_STOCK_V010: &str =
    "bitcoin-4b1bc93ea7f03c49c4424b56561c9c7437e5f16e5714cece615a48e249264a84.c15";
const RGB_TRANSFER_FILE_V010: &str =
    "bitcoin-28cc77c6e8e65def696101d839d0f335effa5da22d4a9c6d843bc6caa87e957e.c15";

const MAINNET_WALLET_V011: &str =
    "bitcoin-c6a76b77e373e787fb4c2d5ede6d8e08a391d2736c79774e92b9182a668c2016.c15";
const TESTNET_WALLET_V011: &str =
    "testnet-c6a76b77e373e787fb4c2d5ede6d8e08a391d2736c79774e92b9182a668c2016.c15";
const SIGNET_WALLET_V011: &str =
    "signet-c6a76b77e373e787fb4c2d5ede6d8e08a391d2736c79774e92b9182a668c2016.c15";
const REGTEST_WALLET_V011: &str =
    "regtest-c6a76b77e373e787fb4c2d5ede6d8e08a391d2736c79774e92b9182a668c2016.c15";
const RGB_STOCK_V011: &str =
    "bitcoin-b495d5c8e5ec8dc19ed3bd192a4c29241b5a062e301e8f1bf29f58788413d277.c15";
const RGB_TRANSFER_FILE_V011: &str =
    "bitcoin-628b74d95fbf42c85f07c471b24e086773b6bf697f422329782bd269ea00a23a.c15";

const MAINNET_WALLET_V011B4: &str =
    "bitcoin-37721699270e009abf5193119008e2d450eb42175b67646aed124d28349456de.c15";
const TESTNET_WALLET_V011B4: &str =
    "testnet-37721699270e009abf5193119008e2d450eb42175b67646aed124d28349456de.c15";
const SIGNET_WALLET_V011B4: &str =
    "signet-37721699270e009abf5193119008e2d450eb42175b67646aed124d28349456de.c15";
const REGTEST_WALLET_V011B4: &str =
    "regtest-37721699270e009abf5193119008e2d450eb42175b67646aed124d28349456de.c15";
const RGB_STOCK_V011B4: &str =
    "bitcoin-01dac05de1cb299bdd8f1588956bffce3c4bade70212914ebd0c26361760d19b.c15";
const RGB_TRANSFER_FILE_V011B4: &str =
    "bitcoin-595a04dff9427f40dac325847bf68bbb8ddbc172db5838e1e2dc8e8ec86ae628.c15";

const MAINNET_WALLET_V011B5: &str =
    "bitcoin-a9a043d606072a56fb05ce29c84891f2851809202a5b3d2b43a4de67ec3133b7.c15";
const TESTNET_WALLET_V011B5: &str =
    "testnet-a9a043d606072a56fb05ce29c84891f2851809202a5b3d2b43a4de67ec3133b7.c15";
const SIGNET_WALLET_V011B5: &str =
    "signet-a9a043d606072a56fb05ce29c84891f2851809202a5b3d2b43a4de67ec3133b7.c15";
const REGTEST_WALLET_V011B5: &str =
    "regtest-a9a043d606072a56fb05ce29c84891f2851809202a5b3d2b43a4de67ec3133b7.c15";
const RGB_STOCK_V011B5: &str =
    "bitcoin-19222543556e1b0b944d34dd74a7adb7588149f3caedd9ac07423c452c7aeb65.c15";
const RGB_TRANSFER_FILE_V011B5: &str =
    "bitcoin-4a155293c68f3b0ddb31668e9089ad9a94be3ba0b13d125e3dea38d67c9b2f67.c15";

const MAINNET_WALLET_V011B6: &str =
    "bitcoin-8dc01c81171285f521e2e04d951e9252f9e966e160daf2796630bf851dacea00.c15";
const TESTNET_WALLET_V011B6: &str =
    "testnet-8dc01c81171285f521e2e04d951e9252f9e966e160daf2796630bf851dacea00.c15";
const SIGNET_WALLET_V011B6: &str =
    "signet-8dc01c81171285f521e2e04d951e9252f9e966e160daf2796630bf851dacea00.c15";
const REGTEST_WALLET_V011B6: &str =
    "regtest-8dc01c81171285f521e2e04d951e9252f9e966e160daf2796630bf851dacea00.c15";
const RGB_STOCK_V011B6: &str =
    "bitcoin-6ea63c5e450a6a0012073eea01b30d2c98e17f619130679b3a163004d4176e9d.c15";
const RGB_TRANSFER_FILE_V011B6: &str =
    "bitcoin-497b4796957d8e50a6cdbae5276b123fcac5673b44c1d2d319dd486c19a29af7.c15";

const NETWORK_BITCOIN: &str = "bitcoin";
const NETWORK_TESTNET: &str = "testnet";
const NETWORK_SIGNET: &str = "signet";
const NETWORK_REGTEST: &str = "regtest";
const NETWORK_TOTAL: &str = "total";
const NETWORK_RGB_STOCKS: &str = "rgb_stocks";
const NETWORK_RGB_TRANSFER_FILES: &str = "rgb_transfer_files";

const DB_PATHS: &str = "PATHS";
const DB_DAYS: &str = "DAYS";
/// Conservative default LMDB map size (1 GiB) for bitmaskd metrics PATHS/DAYS.
const METRICS_MAP_SIZE: usize = 1024 * 1024 * 1024;

type BytesDb = Database<Bytes, Bytes>;

struct MetricsKv {
    env: Env,
    paths: BytesDb,
    days: BytesDb,
}

impl MetricsKv {
    fn insert_paths(&self, key: &[u8], val: &[u8]) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.paths.put(&mut wtxn, key, val)?;
        wtxn.commit()?;
        Ok(())
    }

    fn contains_paths(&self, key: &[u8]) -> Result<bool> {
        let rtxn = self.env.read_txn()?;
        Ok(self.paths.get(&rtxn, key)?.is_some())
    }

    fn insert_days(&self, key: &[u8], val: &[u8]) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.days.put(&mut wtxn, key, val)?;
        wtxn.commit()?;
        Ok(())
    }

    fn contains_days(&self, key: &[u8]) -> Result<bool> {
        let rtxn = self.env.read_txn()?;
        Ok(self.days.get(&rtxn, key)?.is_some())
    }
}

fn unique_temp_metrics_heed_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    env::temp_dir().join(format!(
        "bitmaskd-metrics-heed-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn open_metrics_kv(path: &Path) -> Result<MetricsKv> {
    std::fs::create_dir_all(path)?;
    let mut options = EnvOpenOptions::new();
    options.map_size(METRICS_MAP_SIZE).max_dbs(2);
    // SAFETY: LMDB mmap; open is unsafe if the mapped file is mutated out of process.
    let env = unsafe { options.open(path)? };
    let mut wtxn = env.write_txn()?;
    let paths = env.create_database(&mut wtxn, Some(DB_PATHS))?;
    let days = env.create_database(&mut wtxn, Some(DB_DAYS))?;
    wtxn.commit()?;
    Ok(MetricsKv { env, paths, days })
}

static METRICS_DB: Lazy<MetricsKv> = Lazy::new(|| {
    let path =
        PathBuf::from(env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned()))
            .join("metrics_heed_kv");
    open_metrics_kv(&path).unwrap_or_else(|e| {
        error!("Trouble opening heed metrics store: {e}. Using a temporary directory.");
        let tmp = unique_temp_metrics_heed_dir();
        #[allow(clippy::expect_used)]
        open_metrics_kv(&tmp).expect("temporary heed metrics store")
    })
});

pub async fn init(dir: &Path) -> Result<()> {
    let mut metrics = MetricsData::default();

    metrics
        .wallets_by_network
        .insert(NETWORK_BITCOIN.to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_TESTNET.to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_SIGNET.to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_REGTEST.to_string(), 0);
    metrics.wallets_by_network.insert("total".to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_RGB_STOCKS.to_string(), 0);
    metrics
        .wallets_by_network
        .insert(NETWORK_RGB_TRANSFER_FILES.to_string(), 0);

    let mut total_wallets = 0;
    let mut rgb_stocks = 0;
    let mut rgb_transfer_files = 0;

    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().to_string();
        let metadata = entry.metadata()?;
        let day_created = metadata.created()?;
        let day = round_datetime_to_day(day_created.into());

        METRICS_DB.insert_paths(entry.path().to_str().unwrap_or("ERROR").as_bytes(), &[1])?;

        if metadata.is_file() {
            metrics.bytes += metadata.len();

            *metrics.bytes_by_day.entry(day.clone()).or_insert(0) += metadata.len();

            match filename.as_str() {
                MAINNET_WALLET_V010
                | MAINNET_WALLET_V011
                | MAINNET_WALLET_V011B4
                | MAINNET_WALLET_V011B5
                | MAINNET_WALLET_V011B6 => {
                    *metrics
                        .wallets_by_network
                        .get_mut(NETWORK_BITCOIN)
                        .unwrap_or(&mut 0) += 1;
                    *metrics
                        .bitcoin_wallets_by_day
                        .entry(day.clone())
                        .or_insert(0) += 1;
                    total_wallets += 1;
                }

                TESTNET_WALLET_V010
                | TESTNET_WALLET_V011
                | TESTNET_WALLET_V011B4
                | TESTNET_WALLET_V011B5
                | TESTNET_WALLET_V011B6 => {
                    *metrics
                        .wallets_by_network
                        .get_mut(NETWORK_TESTNET)
                        .unwrap_or(&mut 0) += 1;
                    *metrics
                        .testnet_wallets_by_day
                        .entry(day.clone())
                        .or_insert(0) += 1;
                    total_wallets += 1;
                }

                SIGNET_WALLET_V010 | SIGNET_WALLET_V011 | SIGNET_WALLET_V011B4
                | SIGNET_WALLET_V011B5 | SIGNET_WALLET_V011B6 => {
                    *metrics
                        .wallets_by_network
                        .get_mut(NETWORK_SIGNET)
                        .unwrap_or(&mut 0) += 1;
                    *metrics
                        .signet_wallets_by_day
                        .entry(day.clone())
                        .or_insert(0) += 1;
                    total_wallets += 1;
                }

                REGTEST_WALLET_V010
                | REGTEST_WALLET_V011
                | REGTEST_WALLET_V011B4
                | REGTEST_WALLET_V011B5
                | REGTEST_WALLET_V011B6 => {
                    *metrics
                        .wallets_by_network
                        .get_mut(NETWORK_REGTEST)
                        .unwrap_or(&mut 0) += 1;
                    *metrics
                        .regtest_wallets_by_day
                        .entry(day.clone())
                        .or_insert(0) += 1;
                    total_wallets += 1;
                }

                RGB_STOCK_V010 | RGB_STOCK_V011 | RGB_STOCK_V011B4 | RGB_STOCK_V011B5
                | RGB_STOCK_V011B6 => {
                    rgb_stocks += 1;
                }

                RGB_TRANSFER_FILE_V010
                | RGB_TRANSFER_FILE_V011
                | RGB_TRANSFER_FILE_V011B4
                | RGB_TRANSFER_FILE_V011B5
                | RGB_TRANSFER_FILE_V011B6 => {
                    rgb_transfer_files += 1;
                }

                _ => {}
            }
        }
    }

    *metrics
        .wallets_by_network
        .get_mut(NETWORK_TOTAL)
        .unwrap_or(&mut 0) = total_wallets;

    *metrics
        .wallets_by_network
        .get_mut(NETWORK_RGB_STOCKS)
        .unwrap_or(&mut 0) = rgb_stocks;

    *metrics
        .wallets_by_network
        .get_mut(NETWORK_RGB_TRANSFER_FILES)
        .unwrap_or(&mut 0) = rgb_transfer_files;

    #[allow(clippy::expect_used)]
    let start_day = DateTime::<Utc>::from_naive_utc_and_offset(
        NaiveDate::from_ymd_opt(2023, 7, 25)
            .expect("correct date")
            .and_hms_opt(0, 0, 0)
            .expect("correct time"),
        Utc,
    );
    let end_day: DateTime<Utc> = SystemTime::now().into();
    let end_day = round_datetime_to_day(end_day);

    let mut d = 0;

    loop {
        let start_day = start_day + Duration::days(d);
        let day_prior = start_day - Duration::days(1);
        let day_prior = round_datetime_to_day(day_prior);
        let day = round_datetime_to_day(start_day);

        let bytes_day_prior = {
            metrics
                .bytes_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
            .bytes_by_day
            .entry(day.clone())
            .and_modify(|b| *b += bytes_day_prior)
            .or_insert(bytes_day_prior);

        let bitcoin_wallets_day_prior = {
            metrics
                .bitcoin_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
            .bitcoin_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += bitcoin_wallets_day_prior)
            .or_insert(bitcoin_wallets_day_prior);

        let testnet_wallets_day_prior = {
            metrics
                .testnet_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
            .testnet_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += testnet_wallets_day_prior)
            .or_insert(testnet_wallets_day_prior);

        let signet_wallets_day_prior = {
            metrics
                .signet_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
            .signet_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += signet_wallets_day_prior)
            .or_insert(signet_wallets_day_prior);

        let regtest_wallets_day_prior = {
            metrics
                .regtest_wallets_by_day
                .get(&day_prior)
                .unwrap_or(&0)
                .to_owned()
        };

        metrics
            .regtest_wallets_by_day
            .entry(day.clone())
            .and_modify(|w| *w += regtest_wallets_day_prior)
            .or_insert(regtest_wallets_day_prior);

        if day == end_day {
            break;
        } else {
            d += 1;
        }
    }

    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());

    fs::write(&format!("{dir}/metrics.json"), &json(&metrics).await?)
        .await
        .expect("writing metrics fail");
    fs::write(&format!("{dir}/metrics.csv"), &csv(&metrics).await)
        .await
        .expect("writing metrics fail");

    Ok(())
}

pub async fn update(path: &Path) -> Result<()> {
    debug!("Updating metrics with {path:?}");

    if METRICS_DB.contains_paths(path.to_str().unwrap_or("ERROR").as_bytes())? {
        debug!("Path already present");
        return Ok(());
    } else {
        METRICS_DB.insert_paths(path.to_str().unwrap_or("ERROR").as_bytes(), &[1])?;
    }

    let filename = path
        .file_name()
        .ok_or(anyhow!("no filename for path"))?
        .to_string_lossy()
        .to_string();
    let metadata = path.metadata()?;
    let day_created = metadata.created()?;
    #[allow(clippy::expect_used)]
    let day_prior = day_created
        .checked_sub(Duration::days(1).to_std()?)
        .expect("day exists");
    let day_prior = round_datetime_to_day(day_prior.into());
    let day = round_datetime_to_day(day_created.into());

    let first_of_day = if METRICS_DB.contains_days(day.as_bytes())? {
        debug!("Day already present");
        false
    } else {
        METRICS_DB.insert_days(day.as_bytes(), &[1])?;
        true
    };

    let carbonado_dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());
    let metrics_json = format!("{carbonado_dir}/metrics.json");
    let metrics_csv = format!("{carbonado_dir}/metrics.csv");

    let mut metrics: MetricsData = serde_json::from_str(&fs::read_to_string(&metrics_json).await?)?;

    if metadata.is_file() {
        if first_of_day {
            let bytes_day_prior = {
                metrics
                    .bytes_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .bytes_by_day
                .entry(day.clone())
                .and_modify(|b| *b += bytes_day_prior)
                .or_insert(bytes_day_prior);

            let bitcoin_wallets_day_prior = {
                metrics
                    .bitcoin_wallets_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .bitcoin_wallets_by_day
                .entry(day.clone())
                .and_modify(|w| *w += bitcoin_wallets_day_prior)
                .or_insert(bitcoin_wallets_day_prior);

            let testnet_wallets_day_prior = {
                metrics
                    .testnet_wallets_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .testnet_wallets_by_day
                .entry(day.clone())
                .and_modify(|w| *w += testnet_wallets_day_prior)
                .or_insert(testnet_wallets_day_prior);

            let signet_wallets_day_prior = {
                metrics
                    .signet_wallets_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .signet_wallets_by_day
                .entry(day.clone())
                .and_modify(|w| *w += signet_wallets_day_prior)
                .or_insert(signet_wallets_day_prior);

            let regtest_wallets_day_prior = {
                metrics
                    .regtest_wallets_by_day
                    .get(&day_prior)
                    .unwrap_or(&0)
                    .to_owned()
            };

            metrics
                .regtest_wallets_by_day
                .entry(day.clone())
                .and_modify(|w| *w += regtest_wallets_day_prior)
                .or_insert(regtest_wallets_day_prior);
        }

        metrics.bytes += metadata.len();

        *metrics.bytes_by_day.entry(day.clone()).or_insert(0) += metadata.len();

        match filename.as_str() {
            MAINNET_WALLET_V010
            | MAINNET_WALLET_V011
            | MAINNET_WALLET_V011B4
            | MAINNET_WALLET_V011B5
            | MAINNET_WALLET_V011B6 => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_BITCOIN)
                    .unwrap_or(&mut 0) += 1;
                *metrics
                    .bitcoin_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TOTAL)
                    .unwrap_or(&mut 0) += 1;
            }
            TESTNET_WALLET_V010
            | TESTNET_WALLET_V011
            | TESTNET_WALLET_V011B4
            | TESTNET_WALLET_V011B5
            | TESTNET_WALLET_V011B6 => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TESTNET)
                    .unwrap_or(&mut 0) += 1;
                *metrics
                    .testnet_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TOTAL)
                    .unwrap_or(&mut 0) += 1;
            }
            SIGNET_WALLET_V010 | SIGNET_WALLET_V011 | SIGNET_WALLET_V011B4
            | SIGNET_WALLET_V011B5 | SIGNET_WALLET_V011B6 => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_SIGNET)
                    .unwrap_or(&mut 0) += 1;
                *metrics
                    .signet_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TOTAL)
                    .unwrap_or(&mut 0) += 1;
            }
            REGTEST_WALLET_V010
            | REGTEST_WALLET_V011
            | REGTEST_WALLET_V011B4
            | REGTEST_WALLET_V011B5
            | REGTEST_WALLET_V011B6 => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_REGTEST)
                    .unwrap_or(&mut 0) += 1;
                *metrics
                    .regtest_wallets_by_day
                    .entry(day.clone())
                    .or_insert(0) += 1;
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_TOTAL)
                    .unwrap_or(&mut 0) += 1;
            }

            RGB_STOCK_V010 | RGB_STOCK_V011 | RGB_STOCK_V011B4 | RGB_STOCK_V011B5 => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_RGB_STOCKS)
                    .unwrap_or(&mut 0) += 1;
            }

            RGB_TRANSFER_FILE_V010
            | RGB_TRANSFER_FILE_V011
            | RGB_TRANSFER_FILE_V011B4
            | RGB_TRANSFER_FILE_V011B5
            | RGB_TRANSFER_FILE_V011B6 => {
                *metrics
                    .wallets_by_network
                    .get_mut(NETWORK_RGB_TRANSFER_FILES)
                    .unwrap_or(&mut 0) += 1;
            }

            _ => {}
        }
    }

    // Write metrics beside the carbonado dir, not under metrics.json.
    fs::write(&metrics_json, &json(&metrics).await?).await?;
    fs::write(&metrics_csv, &csv(&metrics).await).await?;

    Ok(())
}

pub async fn csv(metrics: &MetricsData) -> String {
    let mut lines = vec![vec![
        "Wallet".to_owned(),
        "Wallet Count".to_owned(),
        "Bytes Total".to_owned(),
        "Day".to_owned(),
        "Bitcoin Wallets by Day".to_owned(),
        "Testnet Wallets by Day".to_owned(),
        "Signet Wallets by Day".to_owned(),
        "Regtest Wallets by Day".to_owned(),
        "Bytes by Day".to_owned(),
    ]];

    for (day, bitcoin_wallets) in metrics.bitcoin_wallets_by_day.iter() {
        let mut line = vec![];

        if lines.len() == 1 {
            line.push(NETWORK_BITCOIN.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_BITCOIN)
                    .unwrap_or(&0)
                    .to_string(),
            );
            line.push(metrics.bytes.to_string());
        }

        if lines.len() == 2 {
            line.push(NETWORK_TESTNET.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_TESTNET)
                    .unwrap_or(&0)
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 3 {
            line.push(NETWORK_SIGNET.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_SIGNET)
                    .unwrap_or(&0)
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 4 {
            line.push(NETWORK_REGTEST.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_REGTEST)
                    .unwrap_or(&0)
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 5 {
            line.push(NETWORK_TOTAL.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_TOTAL)
                    .unwrap_or(&0)
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 6 {
            line.push(NETWORK_RGB_STOCKS.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_RGB_STOCKS)
                    .unwrap_or(&0)
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() == 7 {
            line.push(NETWORK_RGB_TRANSFER_FILES.to_string());
            line.push(
                metrics
                    .wallets_by_network
                    .get(NETWORK_RGB_TRANSFER_FILES)
                    .unwrap_or(&0)
                    .to_string(),
            );
            line.push("".to_owned());
        }

        if lines.len() > 7 {
            line.push("".to_owned());
            line.push("".to_owned());
            line.push("".to_owned());
        }

        line.push(day.clone());
        line.push(bitcoin_wallets.to_string());
        line.push(
            metrics
                .testnet_wallets_by_day
                .get(day)
                .unwrap_or(&0)
                .to_string(),
        );
        line.push(
            metrics
                .signet_wallets_by_day
                .get(day)
                .unwrap_or(&0)
                .to_string(),
        );
        line.push(
            metrics
                .regtest_wallets_by_day
                .get(day)
                .unwrap_or(&0)
                .to_string(),
        );
        line.push(metrics.bytes_by_day.get(day).unwrap_or(&0).to_string());

        lines.push(line);
    }

    let lines: Vec<String> = lines.iter().map(|line| line.join(",")).collect();
    lines.join("\n")
}

pub async fn json(metrics: &MetricsData) -> Result<String> {
    Ok(serde_json::to_string_pretty(metrics)?)
}

fn round_datetime_to_day(datetime: DateTime<Utc>) -> String {
    #[allow(clippy::expect_used)]
    let rounded = datetime
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("valid time");
    rounded.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Named TDD contract: a temp heed env accepts a PATHS key and reads it back.
    #[test]
    fn paths_key_roundtrip_on_temp_heed_env() {
        let dir = std::env::temp_dir().join(format!(
            "bitmask-metrics-heed-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp metrics dir");
        let store = open_metrics_kv(&dir).expect("open heed env");
        let key = b"/tmp/example/wallet.c15";
        store.insert_paths(key, &[1]).expect("put PATHS");
        let rtxn = store.env.read_txn().expect("read txn");
        let got = store.paths.get(&rtxn, key).expect("get PATHS");
        assert_eq!(got, Some(&[1][..]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_metrics_kv_on_distinct_temp_dirs_do_not_share_keys() {
        let dir_a = unique_temp_metrics_heed_dir();
        let dir_b = unique_temp_metrics_heed_dir();
        assert_ne!(dir_a, dir_b);
        assert_ne!(
            dir_a,
            std::env::temp_dir().join("bitmaskd-metrics-heed-tmp")
        );
        let store_a = open_metrics_kv(&dir_a).expect("open A");
        let store_b = open_metrics_kv(&dir_b).expect("open B");
        store_a.insert_paths(b"only-a", &[1]).expect("put A");
        assert!(store_a.contains_paths(b"only-a").expect("contains A"));
        assert!(
            !store_b
                .contains_paths(b"only-a")
                .expect("B must not see A's key")
        );
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }

    /// Named TDD contract: update writes metrics.json and metrics.csv beside
    /// the carbonado dir, not under metrics.json as a directory.
    #[tokio::test]
    async fn update_writes_metrics_files_beside_carbonado_dir_not_under_metrics_json() {
        let carbonado_dir = unique_temp_metrics_heed_dir();
        std::fs::create_dir_all(&carbonado_dir).expect("carbonado dir");
        let seed = MetricsData::default();
        std::fs::write(
            carbonado_dir.join("metrics.json"),
            serde_json::to_string_pretty(&seed).expect("seed json"),
        )
        .expect("seed metrics.json");
        let new_file = carbonado_dir.join("new-blob.c15");
        std::fs::write(&new_file, b"hello-metrics").expect("new file");

        // SAFETY: other tests in this binary do not read CARBONADO_DIR.
        unsafe {
            std::env::set_var("CARBONADO_DIR", &carbonado_dir);
        }

        update(&new_file)
            .await
            .expect("update writes beside carbonado dir");

        let json_path = carbonado_dir.join("metrics.json");
        let csv_path = carbonado_dir.join("metrics.csv");
        assert!(json_path.is_file(), "metrics.json stays a file");
        assert!(
            !json_path.is_dir(),
            "metrics.json must not become a directory"
        );
        assert!(csv_path.is_file(), "metrics.csv lands beside carbonado dir");
        assert!(
            !json_path.join("metrics.json").exists(),
            "must not write json under metrics.json as a directory"
        );
        assert!(
            !json_path.join("metrics.csv").exists(),
            "must not write csv under metrics.json as a directory"
        );
        let _ = std::fs::remove_dir_all(&carbonado_dir);
    }
}
