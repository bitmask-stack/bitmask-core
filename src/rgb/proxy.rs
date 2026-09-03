use crate::debug;
use crate::proxy::{
    ProxyServerError, proxy_consig_retrieve, proxy_consig_store, proxy_media_data_store,
    proxy_metadata_retrieve,
};
use crate::rgb::TransferError;
use crate::rgb::contract::extract_transfer;
use crate::structs::RgbInvoiceValue;
use crate::validators::RGBValidationOptions;
use armor::AsciiArmor;
use garde::Validate;
use postcard::from_bytes;
use rgbinvoice::{Beneficiary, RgbInvoice};
use rgbstd::containers::Transfer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum ProxyError {
    /// Proxy Server error. {0}
    IO(ProxyServerError),
    /// Consignment cannot be decoded. {0}
    WrongConsig(String),
    /// Retrieve '{0}' serialize causes error. {1}
    SerializeRetrieve(String, String),
    /// Write '{0}' serialize causes error. {1}
    SerializeWrite(String, String),
    /// Amplify hex error. {0}
    AmplifyHexError(#[from] amplify::hex::Error),
    /// Amplify confinement error. {0}
    AmplifyConfinementError(#[from] amplify::confinement::Error),
    /// Strict encoding deserialize errror. {0}
    StenDeserializeError(#[from] strict_encoding::DeserializeError),
    /// No response from proxy
    NoResponseError,
    /// JSON RPC Parse error. {0}
    Parse(String),
}

pub type RgbProxyConsigCarbonadoReq = RgbProxyCarbonadoReq<RgbProxyConsigUpload>;
pub type RgbProxyConsigUploadReq = RgbProxyUploadReq<RgbProxyConsigUpload>;
pub type RgbProxyConsigFileReq = RgbProxyFileReq<RgbProxyConsigUpload>;

pub type RgbProxyConsigUploadRes = RgbProxyRes<bool>;
pub type RgbProxyConsigRes = RgbProxyRes<Option<RgbProxyConsig>>;
pub type RgbProxyConsigErrorRes = RgbProxyErrorRes<String>;

pub type RgbProxyMediaCarbonadoReq = RgbProxyCarbonadoReq<RgbProxyMedia>;
pub type RgbProxyMediaUploadReq = RgbProxyUploadReq<RgbProxyMedia>;
pub type RgbProxyMediaUploadRes = RgbProxyRes<bool>;
pub type RgbProxyMediaFileReq = RgbProxyFileReq<RgbProxyMedia>;
pub type RgbProxyMediaReq = RgbProxyMedia;
pub type RgbProxyMediaRes = RgbProxyRes<String>;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyMedia {
    pub attachment_id: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyConsigReq {
    pub recipient_id: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]

pub struct RgbProxyRes<T> {
    pub jsonrpc: String,
    pub id: String,
    pub result: T,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]

pub struct RgbProxyErrorRes<T> {
    pub jsonrpc: String,
    pub id: String,
    pub error: T,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyFileReq<T> {
    pub params: T,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyCarbonadoReq<T> {
    pub params: T,
    pub file_name: String,
    pub hex: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyUploadReq<T> {
    pub params: T,
    pub file_name: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyConsigUpload {
    pub recipient_id: String,
    pub txid: String,
    pub sender_amount: Option<RgbInvoiceValue>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgbProxyConsig {
    pub consignment: String,
    pub txid: String,
    pub sender_amount: Option<RgbInvoiceValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ContractMedia {
    /// Preview Media (in base64)
    pub preview: Option<ContractMediaItem>,
    /// Media (in sha2)
    pub media: Option<ContractMediaItem>,
    /// Attachments (in blake3)
    pub attachments: Vec<ContractMediaItem>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Default)]
pub struct ContractMediaItem {
    /// Media Type
    pub ty: String,
    /// Media Source
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MediaExtractRequest {
    pub encode: MediaEncode,
    pub item: ContractMediaItem,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaPublished {
    pub preview: Option<MediaItem>,
    pub media: Option<MediaItem>,
    pub attachments: Vec<MediaItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub enum MediaEncode {
    #[default]
    #[serde(rename = "base64")]
    Base64,
    #[serde(rename = "sha2")]
    Sha2,
    #[serde(rename = "blake3")]
    Blake3,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[derive(Validate)]
#[garde(context(RGBValidationOptions))]
pub struct MediaItem {
    /// Media ID
    #[garde(ascii)]
    #[garde(length(min = u16::MIN.into(), max = u16::MAX.into()))]
    pub id: String,
    /// Media Type
    #[serde(rename = "type")]
    #[garde(ascii)]
    #[garde(length(min = 1, max = 64))]
    pub ty: String,
    /// Media Encoded Representation
    #[garde(ascii)]
    #[garde(length(min = u16::MIN.into(), max = u16::MAX.into()))]
    pub source: String,
    /// Media Encoded Type
    #[garde(skip)]
    pub encode: MediaEncode,
}

impl MediaItem {
    pub fn new(metadata: MediaMetadata, encode: MediaEncode) -> Self {
        Self {
            id: metadata.id,
            ty: metadata.mime,
            source: metadata.digest,
            encode,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct MediaMetadata {
    pub id: String,
    pub mime: String,
    pub uri: String,
    pub digest: String,
}

impl MediaMetadata {
    pub fn new(id: &str, mime: &str, uri: &str, digest: &str) -> Self {
        Self {
            id: id.to_string(),
            mime: mime.to_string(),
            uri: uri.to_string(),
            digest: digest.to_string(),
        }
    }
}

pub async fn get_media_metadata(media_id: &str) -> Result<Option<MediaMetadata>, ProxyError> {
    let resp = proxy_metadata_retrieve(&format!("{media_id}-metadata"))
        .await
        .map_err(ProxyError::IO)?;

    if resp.is_none() {
        return Ok(None);
    }

    let bytes = base64::decode(&resp.ok_or(ProxyError::NoResponseError)?.result)
        .map_err(|op| ProxyError::SerializeRetrieve("metadata".to_string(), op.to_string()))?;

    if bytes.is_empty() {
        Ok(None)
    } else {
        let metadata: MediaMetadata = from_bytes(&bytes).map_err(|op| {
            ProxyError::SerializeRetrieve("metadata.postcard".to_string(), op.to_string())
        })?;

        Ok(Some(metadata))
    }
}

pub async fn get_consignment(consig_or_receipt_id: &str) -> Result<Option<String>, ProxyError> {
    if let Some((armored, _)) = get_consignment_ex(consig_or_receipt_id).await? {
        Ok(Some(armored))
    } else {
        Ok(None)
    }
}

pub async fn get_consignment_ex(
    recipient_id: &str,
) -> Result<Option<(String, Option<RgbInvoiceValue>)>, ProxyError> {
    let response = proxy_consig_retrieve(recipient_id)
        .await
        .map_err(ProxyError::IO)?;

    match response {
        Some(proxy_res) => {
            if let Some(result) = proxy_res.result {
                // The `consignment` field is a base64 string from the proxy. Decode it.
                let decoded_bytes = base64::decode(&result.consignment)
                    .map_err(|e| ProxyError::Parse(format!("Base64 decode error: {}", e)))?;

                // The decoded bytes are the armored string.
                let armored_consignment = String::from_utf8(decoded_bytes)
                    .map_err(|e| ProxyError::Parse(format!("UTF-8 decode error: {}", e)))?;

                Ok(Some((armored_consignment, result.sender_amount)))
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}

pub async fn post_media_metadata(
    data: ContractMediaItem,
    encode: MediaEncode,
) -> Result<MediaMetadata, ProxyError> {
    let data = proxy_media_data_store(data, encode)
        .await
        .map_err(ProxyError::IO)?;

    Ok(data)
}

pub async fn post_media_metadata_list(
    data: Vec<ContractMediaItem>,
    encode: MediaEncode,
) -> Result<Vec<MediaMetadata>, ProxyError> {
    let mut list = vec![];
    for item in data {
        let data: MediaMetadata = proxy_media_data_store(item, encode.clone())
            .await
            .map_err(ProxyError::IO)?;

        list.push(data);
    }

    Ok(list)
}

pub async fn publish_rgb_transfer(
    rgb_invoice: RgbInvoice,
    rgb_transfer: Transfer,
    sender_amount: Option<RgbInvoiceValue>,
) -> Result<(), TransferError> {
    let beneficiaries = match rgb_invoice.beneficiary.into_inner() {
        Beneficiary::BlindedSeal(seal) => {
            let seal = seal.to_string();
            let transfer = rgb_transfer.to_ascii_armored_string();
            bmap! { seal => transfer }
        }
        Beneficiary::WitnessVout(addr, _) => {
            let addr =
                crate::rgb::compat::address_bp_from_pay2vout(addr, rgb_invoice.address_network())
                    .to_string();
            let transfer = rgb_transfer.to_ascii_armored_string();
            bmap! { addr => transfer }
        }
    };

    post_consignments(beneficiaries, sender_amount)
        .await
        .map_err(TransferError::Proxy)?;
    Ok(())
}

// This function is now corrected to send the proper data format.
pub async fn post_consignments(
    consignments: BTreeMap<String, String>,
    sender_amount: Option<RgbInvoiceValue>,
) -> Result<(), ProxyError> {
    for (recipient_id, armored_transfer) in consignments {
        let hashed_name = blake3::hash(recipient_id.as_bytes())
            .to_hex()
            .to_lowercase();

        let parsed_transfer = Transfer::from_str(&armored_transfer)
            .map_err(|err| ProxyError::WrongConsig(err.to_string()))?;

        let (txid, _) = extract_transfer(&parsed_transfer)
            .map_err(|op| ProxyError::WrongConsig(op.to_string()))?;

        let bytes = armored_transfer.into_bytes();

        let consig_rq = RgbProxyConsigFileReq {
            params: RgbProxyConsigUpload {
                recipient_id: recipient_id.clone(),
                txid: txid.to_string(),
                sender_amount: sender_amount.clone(),
            },
            bytes,
            file_name: hashed_name,
        };

        let proxy_resp = proxy_consig_store(consig_rq).await;
        if proxy_resp.is_ok() {
            let transfer_id = parsed_transfer.consignment_id();
            debug!(format!(
                "transfer {transfer_id} proxy registred for {recipient_id}"
            ));
        } else {
            let reason = proxy_resp
                .err()
                .map_or(String::default(), |x| x.to_string());
            debug!(format!("proxy error in transfers step: {reason}"));
        }
    }

    Ok(())
}
