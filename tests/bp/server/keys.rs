#![cfg(not(target_arch = "wasm32"))]

use bpstd::{Xpriv, XprivAccount};
use std::str::FromStr;

#[tokio::test]
async fn create_new_bp_keys() -> anyhow::Result<()> {
    let mut entropy = [0u8; 32];
    getrandom_02::getrandom(&mut entropy)?;

    let xpriv = Xpriv::new_master(true, &entropy);
    let xpriv_restore = Xpriv::from_str(xpriv.to_string().as_str())?;
    println!("xpriv: {:#?}", xpriv_restore);
    Ok(())
}

#[tokio::test]
async fn create_new_bp_account() -> anyhow::Result<()> {
    let mut entropy = [0u8; 32];
    getrandom_02::getrandom(&mut entropy)?;

    let xpriv = Xpriv::new_master(true, &entropy);
    let xaccount = XprivAccount::new_master(xpriv);
    println!("xaccount: {}", xaccount);
    Ok(())
}
