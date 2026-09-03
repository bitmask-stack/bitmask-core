//! RGB Protocol 0.11.1-rc.11 adapters.
//! rgb-consensus uses rust-bitcoin types; bp-wallet still uses bp-std.

use rgbcore::ChainNet;
use rgbcore::validation::{
    ValidationConfig, ValidationError, WitnessOrdProvider, WitnessResolverError,
};
use rgbcore::vm::WitnessOrd;
use rgbinvoice::Pay2Vout;
use rgbstd::bitcoin::KnownHrp;
use rgbstd::containers::{Contract, Transfer, UncheckedContract, UncheckedTransfer};
use rgbstd::{RGB_NATIVE_DERIVATION_INDEX, RGB_TAPRET_DERIVATION_INDEX};
use serde::{Deserialize, Deserializer};
use std::str::FromStr;
use strict_types::TypeSystem;

pub use psrgbt::bp_conversion_utils::{
    address_network_bitcoin_to_bp, address_payload_bitcoin_from_script_pubkey,
    address_payload_bp_from_script_pubkey, network_bp_to_bitcoin, outpoint_bitcoin_to_bp,
    outpoint_bp_to_bitcoin, script_buf_to_script_pubkey, tx_bp_to_bitcoin, txid_bitcoin_to_bp,
    txid_bp_to_bitcoin, untweakedpublickey_to_internal_pk,
};

pub fn vout_bp_to_rgb(vout: bpstd::Vout) -> rgbcore::Vout {
    rgbcore::Vout::from(vout.to_u32())
}

#[allow(dead_code)] // reverse of vout_bp_to_rgb
pub fn vout_rgb_to_bp(vout: rgbcore::Vout) -> bpstd::Vout {
    bpstd::Vout::from_u32(vout.to_u32())
}

pub fn script_pubkey_bp_from_pay2vout(pay2vout: Pay2Vout) -> bpstd::ScriptPubkey {
    script_buf_to_script_pubkey(pay2vout.to_script())
}

pub fn address_bp_from_pay2vout(pay2vout: Pay2Vout, hrp: KnownHrp) -> bpstd::Address {
    bpstd::Address::new(
        address_payload_bp_from_script_pubkey(&script_pubkey_bp_from_pay2vout(pay2vout)),
        address_network_bitcoin_to_bp(hrp),
    )
}

pub fn outpoint_bp_from_output_seal(seal: rgbcore::OutputSeal) -> bpstd::Outpoint {
    outpoint_bitcoin_to_bp(rgbstd::Outpoint::from(seal))
}

pub fn tapret_commitment_rgb_from_dbc(
    commit: dbc::tapret::TapretCommitment,
) -> rgbcore::dbc::tapret::TapretCommitment {
    rgbcore::dbc::tapret::TapretCommitment::from_str(&commit.to_string())
        .expect("dbc and rgbcore tapret commitments share display")
}

pub fn close_method_from_dbc(method: dbc::Method) -> rgbcore::seals::txout::CloseMethod {
    match method {
        dbc::Method::TapretFirst => rgbcore::seals::txout::CloseMethod::TapretFirst,
        dbc::Method::OpretFirst => rgbcore::seals::txout::CloseMethod::OpretFirst,
    }
}

/// WitnessOrdProvider that treats one bitcoin Txid as Tentative.
pub struct TentativeWitness {
    pub witness_id: rgbcore::Txid,
}

impl TentativeWitness {
    pub fn from_bp(txid: bpstd::Txid) -> Self {
        Self {
            witness_id: txid_bp_to_bitcoin(txid),
        }
    }
}

impl WitnessOrdProvider for TentativeWitness {
    fn witness_ord(&self, witness_id: rgbcore::Txid) -> Result<WitnessOrd, WitnessResolverError> {
        assert_eq!(witness_id, self.witness_id);
        Ok(WitnessOrd::Tentative)
    }
}

/// Derivation indexes RGB 0.11 used to expose as `RgbKeychain`.
pub fn rgb_keychain_contains(keychain: impl Into<u8>) -> bool {
    let keychain = keychain.into();
    keychain == RGB_NATIVE_DERIVATION_INDEX as u8 || keychain == RGB_TAPRET_DERIVATION_INDEX as u8
}

pub fn rgb_tapret_keychain_u8() -> u8 {
    RGB_TAPRET_DERIVATION_INDEX as u8
}

pub fn rgb_close_method_keychain_u8(tapret_first: bool) -> u8 {
    if tapret_first {
        RGB_TAPRET_DERIVATION_INDEX as u8
    } else {
        RGB_NATIVE_DERIVATION_INDEX as u8
    }
}

pub fn validation_config(chain_net: ChainNet, trusted_typesystem: TypeSystem) -> ValidationConfig {
    ValidationConfig {
        chain_net,
        safe_height: None,
        trusted_typesystem,
        build_opouts_dag: false,
    }
}

pub fn validation_error_text(err: ValidationError) -> String {
    err.to_string()
}

pub mod contract_serde {
    use super::*;
    use serde::{Serialize, Serializer};

    pub fn serialize<S: Serializer>(contract: &Contract, serializer: S) -> Result<S::Ok, S::Error> {
        Serialize::serialize(contract, serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Contract, D::Error> {
        UncheckedContract::deserialize(deserializer)?
            .into_checked()
            .map_err(serde::de::Error::custom)
    }
}

pub mod transfer_serde {
    use super::*;
    use serde::{Serialize, Serializer};

    pub fn serialize<S: Serializer>(transfer: &Transfer, serializer: S) -> Result<S::Ok, S::Error> {
        Serialize::serialize(transfer, serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Transfer, D::Error> {
        UncheckedTransfer::deserialize(deserializer)?
            .into_checked()
            .map_err(serde::de::Error::custom)
    }
}
