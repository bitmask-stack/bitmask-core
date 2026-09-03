#![cfg(feature = "server")]
#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use bitmask_core::regtest::init_fs;

#[test]
pub fn _init() -> Result<()> {
    init_fs()?;

    Ok(())
}
