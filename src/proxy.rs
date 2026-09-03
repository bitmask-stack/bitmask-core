use std::path::PathBuf;

#[derive(Debug, Clone, Eq, PartialEq, Display, From, Error)]
#[display(doc_comments)]
pub enum ProxyServerError {
    /// I/O or connectivity error. {0}
    IO(String),
    /// Server connectivity error. {0}
    Server(String),
    /// JSON RPC Parse error. {0}
    Parse(String),
    /// All endpoints failed error
    AllEndpointsFailed,
    /// No parent found for {0:?}
    MissingParent(PathBuf),
}

/// Prefix an RGB proxy consignment filename with the product named network
/// (`testnet4`, `mutinynet`), not bitcoin 0.29 Display (`testnet`, `signet`).
pub fn proxy_prefixed_name(name: &str, named_network: &str) -> String {
    let networks = [
        "bitcoin",
        "testnet3",
        "testnet4",
        "testnet",
        "signet",
        "mutinynet",
        "regtest",
    ];
    if networks.into_iter().any(|x| name.contains(x)) {
        name.to_owned()
    } else {
        format!("{named_network}-{name}")
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use server::{
    handle_file, proxy_consig_retrieve, proxy_consig_store, proxy_media_data_store,
    proxy_media_retrieve, proxy_metadata_retrieve,
};

#[cfg(target_arch = "wasm32")]
pub use client::{
    proxy_consig_retrieve, proxy_consig_store, proxy_media_data_store, proxy_media_retrieve,
    proxy_metadata_retrieve,
};

#[cfg(not(target_arch = "wasm32"))]
mod server {
    use super::ProxyServerError;
    use crate::debug;
    use crate::{
        constants::{RGB_PROXY_ENDPOINT, get_network},
        info,
        rgb::proxy::{
            ContractMediaItem, MediaEncode, MediaMetadata, RgbProxyConsigFileReq,
            RgbProxyConsigRes, RgbProxyConsigUploadReq, RgbProxyConsigUploadRes, RgbProxyMedia,
            RgbProxyMediaFileReq, RgbProxyMediaRes, RgbProxyMediaUploadReq, RgbProxyMediaUploadRes,
        },
        util::upload_data,
    };
    use amplify::hex::ToHex;
    use bitcoin::hashes::{Hash, sha256};
    use postcard::to_allocvec;
    use reqwest::multipart::{self, Part};
    use std::path::PathBuf;
    use tokio::fs;

    pub async fn proxy_consig_store(
        request: RgbProxyConsigFileReq,
    ) -> Result<RgbProxyConsigUploadRes, ProxyServerError> {
        let RgbProxyConsigFileReq {
            file_name,
            bytes,
            params,
        } = request;

        let filepath = handle_file(&file_name, bytes.len())
            .await
            .map_err(|op| ProxyServerError::Server(op.to_string()))?;

        fs::write(filepath.clone(), bytes)
            .await
            .map_err(|op| ProxyServerError::Server(op.to_string()))?;

        let request_data = RgbProxyConsigUploadReq {
            params,
            file_name: filepath.to_string_lossy().to_string(),
        };

        let resp = fetch_consignment_post(request_data).await?;
        fs::remove_file(filepath)
            .await
            .map_err(|op| ProxyServerError::Server(op.to_string()))?;

        Ok(resp)
    }

    pub async fn proxy_consig_retrieve(
        request_id: &str,
    ) -> Result<Option<RgbProxyConsigRes>, ProxyServerError> {
        fetch_consignment_get(request_id).await
    }

    pub async fn proxy_media_retrieve(
        attachment_id: &str,
    ) -> Result<Option<RgbProxyMediaRes>, ProxyServerError> {
        fetch_media_get(attachment_id).await
    }

    pub async fn proxy_metadata_retrieve(
        attachment_id: &str,
    ) -> Result<Option<RgbProxyMediaRes>, ProxyServerError> {
        fetch_media_get(attachment_id).await
    }

    pub async fn proxy_media_data_store(
        media: ContractMediaItem,
        encode: MediaEncode,
    ) -> Result<MediaMetadata, ProxyServerError> {
        if let Some(content) = retrieve_data(&media.source).await {
            let (id, source) = match encode {
                MediaEncode::Base64 => {
                    let source = base64::encode(&content);
                    let id = blake3::hash(blake3::hash(&content).as_bytes())
                        .to_hex()
                        .to_string();
                    (id, source)
                }
                MediaEncode::Sha2 => {
                    let source = sha256::Hash::hash(&content);
                    let id = source.to_hex().to_string();
                    (id, source.to_hex())
                }
                MediaEncode::Blake3 => {
                    let source = blake3::hash(&content).to_hex().to_string();
                    let id = source.to_string();
                    (id, source)
                }
            };

            let metadata = MediaMetadata::new(&id, &media.ty, &media.source, &source);

            // Store File
            let attachment_id = id;
            let file_req = RgbProxyMediaFileReq {
                params: RgbProxyMedia {
                    attachment_id: attachment_id.clone(),
                },
                file_name: attachment_id.clone(),
                bytes: content,
            };
            proxy_media_store(file_req).await?;

            // Store Metadata
            let metadata_content =
                to_allocvec(&metadata).map_err(|op| ProxyServerError::Parse(op.to_string()))?;
            let metadata_id = format!("{attachment_id}-metadata");
            let file_req = RgbProxyMediaFileReq {
                params: RgbProxyMedia {
                    attachment_id: metadata_id.clone(),
                },
                file_name: metadata_id.clone(),
                bytes: metadata_content,
            };

            proxy_media_store(file_req).await?;

            Ok(metadata)
        } else {
            Err(ProxyServerError::IO("Media not found".to_string()))
        }
    }

    async fn proxy_media_store(
        request: RgbProxyMediaFileReq,
    ) -> Result<RgbProxyMediaUploadRes, ProxyServerError> {
        let RgbProxyMediaFileReq {
            file_name,
            bytes,
            params,
        } = request;

        let filepath = handle_file(&file_name, bytes.len())
            .await
            .map_err(|op| ProxyServerError::IO(op.to_string()))?;

        fs::write(filepath.clone(), bytes)
            .await
            .map_err(|op| ProxyServerError::IO(op.to_string()))?;

        let request_data = RgbProxyMediaUploadReq {
            params,
            file_name: filepath.to_string_lossy().to_string(),
        };

        let resp = fetch_media_post(request_data).await?;
        fs::remove_file(filepath)
            .await
            .map_err(|op| ProxyServerError::IO(op.to_string()))?;

        Ok(resp)
    }

    pub async fn handle_file(name: &str, bytes: usize) -> Result<PathBuf, ProxyServerError> {
        let named_network = get_network().await;
        let final_name = super::proxy_prefixed_name(name, &named_network);

        let filepath = std::path::Path::new(
            &std::env::var("RGB_PROXY_DIR").unwrap_or("/tmp/bitmaskd/proxy".to_owned()),
        )
        .join(final_name);

        let filedir = filepath
            .parent()
            .ok_or_else(|| ProxyServerError::MissingParent(filepath.clone()))?;
        fs::create_dir_all(filedir)
            .await
            .map_err(|op| ProxyServerError::IO(op.to_string()))?;

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

    async fn fetch_consignment_post(
        request: RgbProxyConsigUploadReq,
    ) -> Result<RgbProxyConsigUploadRes, ProxyServerError> {
        let endpoints = RGB_PROXY_ENDPOINT.read().await.to_string();
        let url = format!("{endpoints}/json-rpc");

        let file_info = fs::read(request.file_name.clone())
            .await
            .unwrap_or_default();

        let params = request.params;
        let body = serde_json::to_string(&params).unwrap_or_default();
        let form = multipart::Form::new()
            .text("method", "consignment.post")
            .text("jsonrpc", "2.0")
            .text("id", params.recipient_id)
            .text("params", body)
            .part("file", Part::bytes(file_info).file_name(request.file_name));

        let (resp, _) = upload_data(&url, form).await.map_err(|_| {
            ProxyServerError::Server(format!("Error sending JSON POST request to {url}"))
        })?;

        let resp =
            serde_json::from_str(&resp).map_err(|op| ProxyServerError::Parse(op.to_string()))?;
        Ok(resp)
    }

    async fn fetch_consignment_get(
        recipient_id: &str,
    ) -> Result<Option<RgbProxyConsigRes>, ProxyServerError> {
        let endpoints = RGB_PROXY_ENDPOINT.read().await.to_string();
        let url = format!("{endpoints}/json-rpc");

        let body = serde_json::json!({
            "method": "consignment.get",
            "jsonrpc": "2.0",
            "id": recipient_id.to_string(),
            "params": {
                "recipient_id": recipient_id.to_string()
            }
        });

        info!("-> fetch_consignment_get: Sending request to {}", url);
        debug!("   | Request body: {}", body.to_string());

        let client = reqwest::Client::new();

        // Send the request and await the response object
        let response_result = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await;

        let response = match response_result {
            Ok(resp) => {
                info!(
                    "   | Received response with status: {}",
                    resp.status().to_string()
                );
                resp
            }
            Err(e) => {
                debug!("   ! FAILED to send request to {}: {}", url, e.to_string());
                return Err(ProxyServerError::Server(format!(
                    "Error sending JSON POST request to {}: {}",
                    url, e
                )));
            }
        };

        // Get the response body as text, handling potential errors
        let resp_text = match response.text().await {
            Ok(text) => {
                debug!("   | Received raw response text: {}", text);
                text
            }
            Err(e) => {
                debug!("   ! FAILED to read response text: {}", e.to_string());
                return Err(ProxyServerError::Parse(format!(
                    "Failed to read response text: {}",
                    e
                )));
            }
        };

        // Now, parse the response text
        let resp_result = serde_json::from_str::<RgbProxyConsigRes>(&resp_text);

        match resp_result {
            Ok(parsed_resp) => {
                info!("<- fetch_consignment_get: Successfully parsed response.");
                Ok(Some(parsed_resp))
            }
            Err(e) => {
                // This is a common case for 404s or other errors that return non-JSON bodies
                debug!(
                    "   | Failed to parse response as RgbProxyConsigRes. Error: {}. This may be expected if no consignment was found.",
                    e.to_string()
                );
                Ok(None)
            }
        }
    }

    async fn fetch_media_get(
        attachment_id: &str,
    ) -> Result<Option<RgbProxyMediaRes>, ProxyServerError> {
        let endpoints = RGB_PROXY_ENDPOINT.read().await.to_string();
        let url = format!("{endpoints}/json-rpc");

        // 1. Create a JSON object for the request body.
        let json_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "media.get",
            "id": attachment_id,
            "params": {
                "attachment_id": attachment_id
            }
        });

        // 2. Use a standard HTTP client to send the request with the correct Content-Type.
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Content-Type", "application/json") // Set the correct header
            .body(json_body.to_string()) // Set the body to the serialized JSON
            .send()
            .await
            .map_err(|e| {
                ProxyServerError::Server(format!(
                    "Error sending JSON POST request to {}: {}",
                    url, e
                ))
            })?;

        // 3. Get the response text and parse it.
        let resp_text = response
            .text()
            .await
            .map_err(|e| ProxyServerError::Parse(e.to_string()))?;
        let resp = serde_json::from_str(&resp_text).ok();

        Ok(resp)
    }

    async fn fetch_media_post(
        request: RgbProxyMediaUploadReq,
    ) -> Result<RgbProxyMediaUploadRes, ProxyServerError> {
        let endpoints = RGB_PROXY_ENDPOINT.read().await.to_string();
        let url = format!("{endpoints}/json-rpc");

        let file_info = fs::read(request.file_name.clone())
            .await
            .unwrap_or_default();

        let params = request.params;
        let body = serde_json::to_string(&params).unwrap_or_default();

        // Create the multipart form with the correct method and id for media.post
        let form = multipart::Form::new()
            .text("method", "media.post")
            .text("jsonrpc", "2.0")
            .text("id", params.attachment_id.clone()) // Use attachment_id for media
            .text("params", body)
            .part("file", Part::bytes(file_info).file_name(request.file_name));

        let (resp, _) = upload_data(&url, form).await.map_err(|_| {
            ProxyServerError::Server(format!("Error sending JSON POST request to {url}"))
        })?;

        let resp =
            serde_json::from_str(&resp).map_err(|op| ProxyServerError::Parse(op.to_string()))?;
        Ok(resp)
    }

    async fn retrieve_data(url: &str) -> Option<Vec<u8>> {
        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .header("Accept", "application/octet-stream")
            .header("Cache-Control", "no-cache")
            .send()
            .await;

        if let Ok(response) = response
            && response.status().as_u16() == 200
            && let Ok(bytes) = response.bytes().await
        {
            return Some(bytes.to_vec());
        }

        None
    }
}

#[cfg(target_arch = "wasm32")]
mod client {
    use crate::proxy::ProxyServerError;
    use crate::rgb::proxy::RgbProxyConsigCarbonadoReq;
    use crate::{
        constants::{BITMASK_ENDPOINT, get_network},
        rgb::proxy::{
            ContractMediaItem, MediaEncode, MediaExtractRequest, MediaMetadata,
            RgbProxyConsigFileReq, RgbProxyConsigRes, RgbProxyConsigUploadRes, RgbProxyMediaRes,
        },
        util::{get, post_json},
    };
    use serde_json::json;

    pub async fn proxy_consig_store(
        request: RgbProxyConsigFileReq,
    ) -> Result<RgbProxyConsigUploadRes, ProxyServerError> {
        let named_network = get_network().await;
        let endpoint = BITMASK_ENDPOINT.read().await.to_string();

        let name = request.clone().file_name;
        let prefixed = super::proxy_prefixed_name(&name, &named_network);
        let url = format!("{endpoint}/proxy/consignment/{prefixed}");
        let body = RgbProxyConsigCarbonadoReq::from(request);
        let (response, _) = post_json(&url, &body.clone())
            .await
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;

        let result = serde_json::from_str::<RgbProxyConsigUploadRes>(&response)
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;
        Ok(result.clone())
    }

    pub async fn proxy_consig_retrieve(
        request_id: &str,
    ) -> Result<Option<RgbProxyConsigRes>, ProxyServerError> {
        let endpoint = BITMASK_ENDPOINT.read().await.to_string();
        let url = format!("{endpoint}/json-rpc");

        let body = json!({
            "jsonrpc": "2.0",
            "method": "consignment.get",
            "id": request_id,
            "params": {
                "recipient_id": request_id
            }
        });

        let (response, status_code) = post_json(&url, &body)
            .await
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;

        if status_code >= 400 {
            return Ok(None);
        }

        let resp = serde_json::from_str::<RgbProxyConsigRes>(&response).ok();

        Ok(resp)
    }

    pub async fn proxy_media_retrieve(
        attachment_id: &str,
    ) -> Result<Option<RgbProxyMediaRes>, ProxyServerError> {
        let endpoint = BITMASK_ENDPOINT.read().await.to_string();
        let url = format!("{endpoint}/json-rpc");

        let body = json!({
            "jsonrpc": "2.0",
            "method": "media.get",
            "id": attachment_id,
            "params": {
                "attachment_id": attachment_id
            }
        });

        let (response, status_code) = post_json(&url, &body)
            .await
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;

        if status_code >= 400 {
            return Ok(None);
        }

        let resp = serde_json::from_str::<RgbProxyMediaRes>(&response).ok();
        Ok(resp)
    }

    pub async fn proxy_metadata_retrieve(
        attachment_id: &str,
    ) -> Result<Option<RgbProxyMediaRes>, ProxyServerError> {
        let endpoint = BITMASK_ENDPOINT.read().await.to_string();
        let url = format!("{endpoint}/json-rpc");

        let body = json!({
            "jsonrpc": "2.0",
            "method": "media.get",
            "id": attachment_id,
            "params": {
                "attachment_id": attachment_id
            }
        });

        let (response, status_code) = post_json(&url, &body)
            .await
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;

        if status_code >= 400 {
            return Ok(None);
        }

        let resp = serde_json::from_str::<RgbProxyMediaRes>(&response).ok();
        Ok(resp)
    }

    pub async fn proxy_media_data_store(
        media: ContractMediaItem,
        encode: MediaEncode,
    ) -> Result<MediaMetadata, ProxyServerError> {
        let endpoint = BITMASK_ENDPOINT.read().await.to_string();
        let url = format!("{endpoint}/proxy/media-metadata");
        let body = MediaExtractRequest {
            encode,
            item: media,
        };
        let (response, _) = post_json(&url, &body.clone())
            .await
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;

        let result = serde_json::from_str::<MediaMetadata>(&response)
            .map_err(|op| ProxyServerError::Parse(op.to_string()))?;
        Ok(result.clone())
    }
}
