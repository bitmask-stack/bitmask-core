use std::str::FromStr;

use bpstd::{Address, Outpoint};
use rgb::OutputSeal;

use crate::structs::{RgbSelection, SecretString};

/// Errors happening during checking of requests to RGB operations
#[derive(Clone, PartialEq, Eq, Debug, Display, Error, From)]
#[display(inner)]
pub enum RGBParamsError {
    /// wrong or unspecified seal closed method. Only TapRet (tapret1st)
    /// is supported
    #[display(doc_comments)]
    WrongCloseMethod,

    /// the {0} need At least {1} media information to create RGB21 contracts
    #[display(doc_comments)]
    NoMediaType(String, u8),

    /// '{0}' is invalid terminal path (ex: /0/0)
    #[display(doc_comments)]
    WrongTerminal(String),

    /// '{0}' is invalid descriptor. {1}
    #[display(doc_comments)]
    WrongDescriptor(String, String),

    /// Rgb Invoice cannot be decoded. {0}
    WrongInvoice(String),
}

#[derive(Debug, Display)]
#[display(doc_comments)]
#[allow(dead_code)]
pub struct RGBValidationOptions {
    // Close Method supported
    closed_method: String,

    // Current Network
    current_network: String,

    // Minimum number of the media types (Only RGB21)
    min_media_types: u8,
}

impl Default for RGBValidationOptions {
    fn default() -> Self {
        Self {
            closed_method: "tapret1st".to_string(),
            current_network: String::new(),
            min_media_types: 1,
        }
    }
}

impl RGBValidationOptions {
    pub fn with(network: &str) -> Self {
        Self {
            current_network: network.to_string(),
            ..Default::default()
        }
    }
}

pub fn pub_desc(_value: &SecretString, _context: &RGBValidationOptions) -> garde::Result {
    Ok(())
}

pub fn rgb_seal(value: &str, context: &RGBValidationOptions) -> garde::Result {
    if !value.contains(&context.closed_method) {
        return Err(garde::Error::new(
            RGBParamsError::WrongCloseMethod.to_string(),
        ));
    }
    OutputSeal::from_str(value).map_err(|op| garde::Error::new(op.to_string()))?;
    Ok(())
}

pub fn rgb_outpoint(value: &str, _context: &RGBValidationOptions) -> garde::Result {
    Outpoint::from_str(value).map_err(|op| garde::Error::new(op.to_string()))?;
    Ok(())
}

pub fn bitcoin_address(value: &str, _context: &RGBValidationOptions) -> garde::Result {
    Address::from_str(value).map_err(|op| garde::Error::new(op.to_string()))?;
    Ok(())
}

pub fn rgb_select_option(value: &RgbSelection, context: &RGBValidationOptions) -> garde::Result {
    match value {
        RgbSelection::UTXO(utxo) => {
            if !utxo.is_empty() {
                rgb_outpoint(utxo, context)?;
            }
        }
        RgbSelection::Address(address) if !address.is_empty() => {
            bitcoin_address(address, context)?;
        }
        _ => {}
    };

    Ok(())
}

pub fn decimal_repr(_value: &str, _context: &RGBValidationOptions) -> garde::Result {
    Ok(())
}
