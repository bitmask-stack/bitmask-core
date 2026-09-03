#![cfg(target_arch = "wasm32")]
use bpwallet::Network;
use wasm_bindgen_test::wasm_bindgen_test_configure;
use wasm_bindgen_test::*;

use bitmask_core::bitcoin::new_mnemonic;
use bitmask_core::debug;
use bitmask_core::structs::SecretString;
use bitmask_core::wallet::interop::{DerivationType, WalletType, XpubKeyBp};

wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn create_interop_key() -> anyhow::Result<()> {
    let network = Network::Regtest;
    let keys = new_mnemonic(&SecretString("".to_string())).await?;

    let interop_key = XpubKeyBp::new(
        &keys.public.rgb_descriptor_xpub,
        network,
        WalletType::Taproot,
        DerivationType::Bitcoin,
    )?;
    debug!(format!("interop key: {interop_key}"));
    Ok(())
}
