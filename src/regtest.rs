#![allow(clippy::panic, clippy::expect_used)] // Allowed because this is only used in testing
#![cfg(not(target_arch = "wasm32"))]
#[cfg(feature = "server")]
use crate::carbonado::metrics::MetricsData;
use std::{env, process::Command};
#[cfg(feature = "server")]
use std::{fs, path};

#[cfg(feature = "server")]
pub fn init_fs() -> anyhow::Result<()> {
    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());
    let dir = path::Path::new(&dir);

    fs::create_dir_all(dir)?;
    fs::write(
        dir.join("metrics.json"),
        serde_json::to_string_pretty(&MetricsData::default())?,
    )?;

    Ok(())
}

pub fn send_coins(address: &str, amount: &str) {
    let path = env::current_dir().expect("oh no!");
    let path = path.to_str().expect("oh no!");
    let full_file = format!("{}/regtest/send_coins.sh", path);

    Command::new("bash")
        .arg(full_file)
        .args([address, amount])
        .output()
        .expect("process spawn");
}

pub fn new_block() {
    let path = env::current_dir().expect("oh no!");
    let path = path.to_str().expect("oh no!");
    let full_file = format!("{}/regtest/new_block.sh", path);

    Command::new("bash")
        .arg(full_file)
        .output()
        .expect("process spawn");
}
