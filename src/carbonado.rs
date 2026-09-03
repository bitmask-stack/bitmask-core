use crate::{carbonado::error::CarbonadoError, constants::NAMED_NETWORK, info};
use amplify::hex::ToHex;
use serde::{Deserialize, Serialize};

pub mod error;

#[cfg(not(target_arch = "wasm32"))]
pub mod metrics;

#[cfg(not(target_arch = "wasm32"))]
pub use server::{
    auctions_retrieve, auctions_store, handle_file, local_store, marketplace_retrieve,
    marketplace_store, retrieve, retrieve_metadata, store,
};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    pub filename: String,
    pub metadata: [u8; 8],
}

fn auction_path_segment(part: &str) -> Result<(), CarbonadoError> {
    if part.is_empty()
        || part.contains('/')
        || part.contains('\\')
        || part.contains('\0')
        || part.contains("..")
    {
        return Err(CarbonadoError::InvalidAuctionBundleId);
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
mod server {
    use crate::{
        constants::{get_coordinator_nostr_key, get_marketplace_nostr_key},
        debug,
    };

    use super::*;

    use std::{
        io::{Error, ErrorKind},
        path::PathBuf,
    };

    use bpstd::secp256k1::{PublicKey, SecretKey};
    use tokio::fs;

    pub async fn store(
        sk: &str,
        name: &str,
        input: &[u8],
        _force: bool,
        metadata: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, CarbonadoError> {
        let level = 15;
        let sk = hex::decode(sk)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.serialize();
        let pk_hex = hex::encode(pk);

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (body, _encode_info) = carbonado::file::encode(&sk, input, level, meta)?;
        let filepath = handle_file(&pk_hex, name, body.len()).await?;
        fs::write(&filepath, &body).await?;
        // metrics::update(&filepath).await?;
        Ok(body)
    }

    pub async fn local_store(
        sk: &str,
        input: &[u8],
        _force: bool,
        metadata: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, CarbonadoError> {
        let level = 15;
        let sk = hex::decode(sk)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let _public_key = PublicKey::from_secret_key_global(&secret_key);

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (body, _) = carbonado::file::encode(&sk, input, level, meta)?;
        // metrics::update(&filepath).await?;
        Ok(body)
    }

    pub async fn marketplace_store(
        name: &str,
        input: &[u8],
        metadata: Option<Vec<u8>>,
    ) -> Result<(PathBuf, Vec<u8>), CarbonadoError> {
        let marketplace_key: String = get_marketplace_nostr_key().await;

        let level = 15;
        let sk = hex::decode(marketplace_key)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.serialize();
        let pk_hex = hex::encode(pk);

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (body, _encode_info) = carbonado::file::encode(&sk, input, level, meta)?;
        let filepath = handle_file(&pk_hex, name, body.len()).await?;
        fs::write(&filepath, body.clone()).await?;
        // metrics::update(&filepath).await?;
        debug!("marketplace encoded data".to_string());
        Ok((filepath, body))
    }

    fn auction_object_name(bundle_id: &str, name: &str) -> Result<String, CarbonadoError> {
        auction_path_segment(bundle_id)?;
        auction_path_segment(name)?;
        Ok(format!("{bundle_id}-{name}"))
    }

    /// RGB 0.11 coordinator auction object. Carbonado 0.7 encode with the
    /// 32-byte coordinator nostr key (same as marketplace_store). bundle_id is
    /// a filename namespace, not an ECDH bundle pubkey.
    pub async fn auctions_store(
        bundle_id: &str,
        name: &str,
        input: &[u8],
        metadata: Option<Vec<u8>>,
    ) -> Result<(PathBuf, Vec<u8>), CarbonadoError> {
        let object_name = auction_object_name(bundle_id, name)?;
        let coordinator_key: String = get_coordinator_nostr_key().await;

        let level = 15;
        let sk = hex::decode(coordinator_key)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.serialize();
        let pk_hex = hex::encode(pk);

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (body, _encode_info) = carbonado::file::encode(&sk, input, level, meta)?;
        let filepath = handle_file(&pk_hex, &object_name, body.len()).await?;
        fs::write(&filepath, body.clone()).await?;
        debug!("auction encoded data".to_string());
        Ok((filepath, body))
    }

    pub async fn retrieve(
        sk: &str,
        name: &str,
        alt_names: Vec<&String>,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        use crate::rgb::constants::RGB_ST_INVALID_VERSION;

        let sk = hex::decode(sk)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let mut final_name = name.to_string();
        let network = NAMED_NETWORK.read().await.to_string();
        let networks = [
            "bitcoin",
            "testnet",
            "testnet3",
            "testnet4",
            "signet",
            "mutinynet",
            "regtest",
        ];
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let filepath = handle_file(&pk, &final_name, 0).await?;
        if let Ok(bytes) = fs::read(filepath).await {
            let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;
            return Ok((decoded, header.metadata.map(|m| m.to_vec())));
        }

        // Check alternative names
        let alt_names = alt_names.into_iter().map(|x| format!("{network}-{x}"));
        for alt_name in alt_names {
            let filepath = handle_file(&pk, &alt_name, 0).await?;
            if let Ok(bytes) = fs::read(filepath).await {
                let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;
                if let Some(metadata) = header.metadata
                    && metadata == RGB_ST_INVALID_VERSION
                {
                    return Ok((decoded, header.metadata.map(|m| m.to_vec())));
                }
            }
        }

        Ok((Vec::new(), None))
    }

    pub async fn marketplace_retrieve(
        name: &str,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        let marketplace_key: String = get_marketplace_nostr_key().await;

        let sk = hex::decode(marketplace_key)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let mut final_name = name.to_string();
        let network = NAMED_NETWORK.read().await.to_string();
        let networks = [
            "bitcoin",
            "testnet",
            "testnet3",
            "testnet4",
            "signet",
            "mutinynet",
            "regtest",
        ];
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let filepath = handle_file(&pk, &final_name, 0).await?;
        if let Ok(bytes) = fs::read(filepath).await {
            let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;

            debug!("marketplace decoded data".to_string());
            return Ok((decoded, header.metadata.map(|m| m.to_vec())));
        }

        Ok((Vec::new(), None))
    }

    pub async fn auctions_retrieve(
        bundle_id: &str,
        name: &str,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        let object_name = auction_object_name(bundle_id, name)?;
        let coordinator_key: String = get_coordinator_nostr_key().await;

        let sk = hex::decode(coordinator_key)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let filepath = handle_file(&pk, &object_name, 0).await?;
        if let Ok(bytes) = fs::read(filepath).await {
            let (header, decoded) = carbonado::file::decode(&sk, &bytes)?;
            debug!("auction decoded data".to_string());
            return Ok((decoded, header.metadata.map(|m| m.to_vec())));
        }

        Ok((Vec::new(), None))
    }

    pub async fn handle_file(
        pk: &str,
        name: &str,
        bytes: usize,
    ) -> Result<PathBuf, CarbonadoError> {
        let mut final_name = name.to_string();
        let network = NAMED_NETWORK.read().await.to_string();
        let networks = [
            "bitcoin",
            "testnet",
            "testnet3",
            "testnet4",
            "signet",
            "mutinynet",
            "regtest",
        ];
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let directory = std::path::Path::new(
            &std::env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned()),
        )
        .join(pk);

        let filepath = directory.join(final_name);
        let filedir = filepath
            .parent()
            .ok_or_else(|| CarbonadoError::MissingParent(filepath.clone()))?;

        fs::create_dir_all(filedir).await.map_err(|_| {
            CarbonadoError::StdIoError(Error::new(
                ErrorKind::NotFound,
                format!("Cannot create filepath to carbonado file {name}"),
            ))
        })?;
        if bytes == 0 {
            info!(format!("read {}", filepath.to_string_lossy()));
        } else {
            info!(format!(
                "write {bytes} bytes to {}",
                filepath.to_string_lossy()
            ));
        }

        Ok(filepath)
    }

    pub async fn retrieve_metadata(sk: &str, name: &str) -> Result<FileMetadata, CarbonadoError> {
        let sk = hex::decode(sk)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let network = NAMED_NETWORK.read().await.to_string();
        let networks = [
            "bitcoin",
            "testnet",
            "testnet3",
            "testnet4",
            "signet",
            "mutinynet",
            "regtest",
        ];

        let mut final_name = name.to_string();
        if !networks.into_iter().any(|x| name.contains(x)) {
            final_name = format!("{network}-{name}");
        }

        let filepath = handle_file(&pk, &final_name, 0).await?;
        let bytes = fs::read(filepath).await?;

        let (header, _) = carbonado::file::decode(&sk, &bytes)?;

        let result = FileMetadata {
            filename: header.file_name(),
            metadata: header.metadata.unwrap_or_default(),
        };

        Ok(result)
    }
}

#[cfg(target_arch = "wasm32")]
pub use client::{
    auctions_retrieve, auctions_store, local_store, marketplace_retrieve, marketplace_store,
    retrieve, retrieve_metadata, store,
};

#[cfg(target_arch = "wasm32")]
mod client {
    use super::*;
    use js_sys::{Array, Promise, Uint8Array};
    use serde::Deserialize;
    use std::sync::Arc;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::{JsFuture, future_to_promise};

    use gloo_net::http::Request;
    use gloo_utils::errors::JsError;

    use crate::{
        constants::{BITMASK_ENDPOINT, CARBONADO_ENDPOINT},
        // rgb::carbonado::FileMetadata,
    };

    use bpstd::secp256k1::{PublicKey, SecretKey};

    fn js_to_error(js_value: JsValue) -> CarbonadoError {
        CarbonadoError::JsError(js_to_js_error(js_value))
    }

    fn js_to_js_error(js_value: JsValue) -> JsError {
        match JsError::try_from(js_value) {
            Ok(error) => error,
            Err(_) => unreachable!("JsValue passed is not an Error type -- this is a bug"),
        }
    }

    #[derive(Debug, Deserialize)]
    struct PostStorePromiseResult {
        value: f64,
    }

    pub async fn store(
        sk: &str,
        name: &str,
        input: &[u8],
        force: bool,
        metadata: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, CarbonadoError> {
        let level = 15;
        let sk = hex::decode(sk)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.serialize();
        let pk_hex = hex::encode(pk);

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (encode_data, _encode_info) = carbonado::file::encode(&sk, input, level, meta)?;
        let body = Arc::new(encode_data.clone());
        let network = NAMED_NETWORK.read().await.to_string();

        let mut force_write = "";
        if force {
            force_write = "/force";
        }

        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();
        let requests = Array::new();
        for endpoint in endpoints {
            let url = format!("{endpoint}/{pk_hex}/{network}-{name}{force_write}");
            let fetch_fn = future_to_promise(fetch_post(url, body.clone())); // TODO: try using .value_of();
            requests.push(&fetch_fn);
        }

        let results = JsFuture::from(Promise::all_settled(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        info!(format!("Store results: {results:?}"));

        let results = serde_wasm_bindgen::from_value::<Vec<PostStorePromiseResult>>(results)?;
        let success = results.iter().any(|result| result.value == 200.0);
        if success {
            Ok(encode_data)
        } else {
            Err(CarbonadoError::AllEndpointsFailed)
        }
    }

    pub async fn local_store(
        sk: &str,
        input: &[u8],
        _force: bool,
        metadata: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, CarbonadoError> {
        let level = 15;
        let sk = hex::decode(sk)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let _public_key = PublicKey::from_secret_key_global(&secret_key);

        let mut meta: Option<[u8; 8]> = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = Some(inner);
        }

        let (body, _) = carbonado::file::encode(&sk, input, level, meta)?;
        Ok(body)
    }

    pub async fn marketplace_store(
        name: &str,
        input: &[u8],
        metadata: Option<Vec<u8>>,
    ) -> Result<(), CarbonadoError> {
        let body = Arc::new(input.to_vec());
        let network = NAMED_NETWORK.read().await.to_string();
        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();
        let requests = Array::new();

        let mut meta: [u8; 8] = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = inner;
        }

        let metadata_version = hex::encode(meta);
        for endpoint in endpoints {
            let url = format!("{endpoint}/server/{metadata_version}/{network}-{name}");
            let fetch_fn = future_to_promise(fetch_post(url, body.clone()));
            requests.push(&fetch_fn);
        }

        let results = JsFuture::from(Promise::all_settled(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        info!(format!("Store results: {results:?}"));

        let results = serde_wasm_bindgen::from_value::<Vec<PostStorePromiseResult>>(results)?;
        let success = results.iter().any(|result| result.value == 200.0);
        if success {
            Ok(())
        } else {
            Err(CarbonadoError::AllEndpointsFailed)
        }
    }

    pub async fn auctions_store(
        bundle_id: &str,
        name: &str,
        input: &[u8],
        metadata: Option<Vec<u8>>,
    ) -> Result<(), CarbonadoError> {
        auction_path_segment(bundle_id)?;
        auction_path_segment(name)?;
        let body = Arc::new(input.to_vec());
        let network = NAMED_NETWORK.read().await.to_string();
        let endpoints = BITMASK_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();
        let requests = Array::new();

        let mut meta: [u8; 8] = default!();
        if let Some(metadata) = metadata {
            let mut inner: [u8; 8] = default!();
            inner[..metadata.len()].copy_from_slice(&metadata);
            meta = inner;
        }

        let metadata_version = hex::encode(meta);
        for endpoint in endpoints {
            let url = format!("{endpoint}/auction/{bundle_id}/{metadata_version}/{network}-{name}");
            let fetch_fn = future_to_promise(fetch_post(url, body.clone()));
            requests.push(&fetch_fn);
        }

        let results = JsFuture::from(Promise::all_settled(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        info!(format!("Store results: {results:?}"));

        let results = serde_wasm_bindgen::from_value::<Vec<PostStorePromiseResult>>(results)?;
        let success = results.iter().any(|result| result.value == 200.0);
        if success {
            Ok(())
        } else {
            Err(CarbonadoError::AllEndpointsFailed)
        }
    }

    pub async fn retrieve(
        sk: &str,
        name: &str,
        alt_names: Vec<&String>,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        use carbonado::file::Header;

        let sk = hex::decode(sk)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let network = NAMED_NETWORK.read().await.to_string();
        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();

        let requests = Array::new();
        for endpoint in endpoints.iter() {
            let url = format!("{endpoint}/{pk}/{network}-{name}");
            let fetch_fn = future_to_promise(fetch_get_byte_array(url));
            requests.push(&fetch_fn);
        }

        let result = JsFuture::from(Promise::any(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        let array = Uint8Array::from(result);
        let encoded = array.to_vec();

        if encoded.len() > Header::LEN {
            let (header, decoded) = carbonado::file::decode(&sk, &encoded)?;
            return Ok((decoded, header.metadata.map(|m| m.to_vec())));
        }

        // Check alternative names
        for alt_name in alt_names {
            let requests = Array::new();
            for endpoint in endpoints.iter() {
                let url = format!("{endpoint}/{pk}/{network}-{alt_name}");

                let fetch_fn = future_to_promise(fetch_get_byte_array(url));
                requests.push(&fetch_fn);
            }

            let result = JsFuture::from(Promise::any(&JsValue::from(requests)))
                .await
                .map_err(js_to_error)?;

            let array = Uint8Array::from(result);
            let encoded = array.to_vec();

            if encoded.len() > Header::LEN {
                let (header, decoded) = carbonado::file::decode(&sk, &encoded)?;
                return Ok((decoded, header.metadata.map(|m| m.to_vec())));
            }
        }

        Ok((Vec::new(), None))
    }

    pub async fn marketplace_retrieve(
        name: &str,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        let network = NAMED_NETWORK.read().await.to_string();
        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();

        let requests = Array::new();
        for endpoint in endpoints.iter() {
            let url = format!("{endpoint}/server/{network}-{name}");
            let fetch_fn = future_to_promise(fetch_get_byte_array(url));
            requests.push(&fetch_fn);
        }

        let result = JsFuture::from(Promise::any(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        let array = Uint8Array::from(result);
        let encoded = array.to_vec();

        Ok((encoded.to_vec(), None))
    }

    pub async fn auctions_retrieve(
        bundle_id: &str,
        name: &str,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), CarbonadoError> {
        auction_path_segment(bundle_id)?;
        auction_path_segment(name)?;
        let network = NAMED_NETWORK.read().await.to_string();
        let endpoints = BITMASK_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();

        let requests = Array::new();
        for endpoint in endpoints.iter() {
            let url = format!("{endpoint}/auction/{bundle_id}/{network}-{name}");
            let fetch_fn = future_to_promise(fetch_get_byte_array(url));
            requests.push(&fetch_fn);
        }

        let result = JsFuture::from(Promise::any(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        let array = Uint8Array::from(result);
        let encoded = array.to_vec();

        Ok((encoded.to_vec(), None))
    }

    pub async fn retrieve_metadata(sk: &str, name: &str) -> Result<FileMetadata, CarbonadoError> {
        let sk = hex::decode(sk)?;
        let secret_key =
            SecretKey::from_slice(&sk).map_err(|_| CarbonadoError::WrongNostrPrivateKey)?;
        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk = public_key.to_hex();

        let network = NAMED_NETWORK.read().await.to_string();
        let endpoints = CARBONADO_ENDPOINT.read().await.to_string();
        let endpoints: Vec<&str> = endpoints.split(',').collect();

        let requests = Array::new();
        for endpoint in endpoints {
            let url = format!("{endpoint}/{pk}/{network}-{name}/metadata");
            let fetch_fn = future_to_promise(fetch_get_text(url));
            requests.push(&fetch_fn);
        }

        let result = JsFuture::from(Promise::any(&JsValue::from(requests)))
            .await
            .map_err(js_to_error)?;

        info!(format!("Retrieve metadata result: {result:?}"));

        let result: FileMetadata =
            serde_json::from_str(&result.as_string().ok_or(CarbonadoError::ResultNotString)?)?;

        Ok(result)
    }

    async fn fetch_post(url: String, body: Arc<Vec<u8>>) -> Result<JsValue, JsValue> {
        let array = Uint8Array::new_with_length(body.len() as u32);
        array.copy_from(&body);

        let request = Request::post(&url)
            .header("Content-Type", "application/octet-stream")
            .header("Cache-Control", "no-cache")
            .body(array);

        let request = match request {
            Ok(request) => request,
            Err(e) => return Err(JsValue::from(e.to_string())),
        };

        let response = request.send().await;

        match response {
            Ok(response) => {
                let status_code = response.status();
                if status_code == 200 {
                    Ok(JsValue::from(status_code))
                } else {
                    Err(JsValue::from(status_code))
                }
            }
            Err(e) => Err(JsValue::from(e.to_string())),
        }
    }

    async fn fetch_get_text(url: String) -> Result<JsValue, JsValue> {
        let request = Request::get(&url)
            .header("Content-Type", "application/octet-stream")
            .header("Cache-Control", "no-cache")
            .build();

        let request = match request {
            Ok(request) => request,
            Err(e) => return Err(JsValue::from(e.to_string())),
        };

        let response = request.send().await;

        match response {
            Ok(response) => {
                let status_code = response.status();
                if status_code == 200 {
                    match response.text().await {
                        Ok(text) => Ok(JsValue::from(&text)),
                        Err(e) => Err(JsValue::from(e.to_string())),
                    }
                } else {
                    Err(JsValue::from(status_code))
                }
            }
            Err(e) => Err(JsValue::from(e.to_string())),
        }
    }

    async fn fetch_get_byte_array(url: String) -> Result<JsValue, JsValue> {
        let request = Request::get(&url)
            .header("Content-Type", "application/octet-stream")
            .header("Cache-Control", "no-cache")
            .build();

        let request = match request {
            Ok(request) => request,
            Err(e) => return Err(JsValue::from(e.to_string())),
        };

        let response = request.send().await;

        match response {
            Ok(response) => {
                let status_code = response.status();
                if status_code == 200 {
                    match response.binary().await {
                        Ok(bytes) => {
                            let array = Uint8Array::new_with_length(bytes.len() as u32);
                            array.copy_from(&bytes);
                            Ok(JsValue::from(&array))
                        }
                        Err(e) => Err(JsValue::from(e.to_string())),
                    }
                } else {
                    Err(JsValue::from(status_code))
                }
            }
            Err(e) => Err(JsValue::from(e.to_string())),
        }
    }
}

// Utility functions for handling data of different encodings
pub mod util {
    use super::*;

    pub fn encode_hex(bytes: &[u8]) -> String {
        hex::encode(bytes)
    }

    pub fn encode_base64(bytes: &[u8]) -> String {
        base64::encode(bytes)
    }

    pub fn decode_hex(string: &str) -> Result<Vec<u8>, CarbonadoError> {
        Ok(hex::decode(string)?)
    }

    pub fn decode_base64(string: &str) -> Result<Vec<u8>, CarbonadoError> {
        Ok(base64::decode(string)?)
    }
}
