#![allow(clippy::nonsensical_open_options)]
#![allow(clippy::suspicious_open_options)]
#![cfg(feature = "server")]
#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, CacheControl, authorization::Bearer},
};

use std::{
    env,
    fs::OpenOptions,
    io::{ErrorKind, Read, Seek, SeekFrom},
    net::SocketAddr,
    path,
    time::Instant,
};

use log::{debug, error, info};
use tokio::{fs, net::TcpListener};
use tower_http::cors::CorsLayer;

use bitmask_core::{
    carbonado::metrics,
    constants::{BMC_VERSION, get_coordinator_nostr_key, get_network, switch_network},
};
use bitmask_core::{
    proxy::{
        proxy_consig_retrieve, proxy_consig_store, proxy_media_data_store, proxy_media_retrieve,
        proxy_metadata_retrieve,
    },
    structs::LoadWalletsRequest,
};
use bitmask_core::{
    rgb::proxy::{MediaExtractRequest, RgbProxyConsigCarbonadoReq, RgbProxyConsigFileReq},
    structs::AtomicMintRequest,
};

use bitmask_core::rgb::issue::IssueContractOptions;

use bitmask_core::carbonado::{
    FileMetadata, auctions_close, auctions_retrieve, auctions_store, handle_file,
    marketplace_retrieve, marketplace_store,
};
use bitmask_core::rgb::transfer::{RgbAcceptOptions, RgbInvoiceOptions, RgbTransferOptions};
use bitmask_core::rgb::{
    accept_transfer, create_invoice, get_contract, import_contract, issue_contract, list_contracts,
    transfer_contract,
};
use bitmask_core::rgb::{clean_public_offers, reset_public_offers};
use bitmask_core::structs::{
    AcceptRequest, IssueRequest, RgbInvoiceRequest, RgbTransferRequest, WatcherRequest,
};
use bitmask_core::wallet::watcher::{create_watcher, watcher_next_address, watcher_next_utxo};
use serde_json::Value;

/// Parse a v2 Carbonado header from in-memory bytes. v2 has no secp256k1 pubkey field.
fn carbonado_header_from_bytes(
    bytes: &[u8],
) -> Result<carbonado::file::Header, carbonado::error::CarbonadoError> {
    carbonado::file::Header::try_from(bytes)
}

/// Read the first [`carbonado::file::Header::LEN`] bytes of an open file and parse a v2 header.
/// `TryFrom<&File>` always errors on Carbonado 0.7 (legacy ECIES path removed).
fn carbonado_header_from_std_file(
    file: &std::fs::File,
) -> Result<carbonado::file::Header, carbonado::error::CarbonadoError> {
    let mut file = file;
    file.seek(SeekFrom::Start(0))?;
    let mut buf = [0u8; carbonado::file::Header::LEN];
    file.read_exact(&mut buf)?;
    carbonado::file::Header::try_from(buf.as_slice())
}

fn carbonado_body_len(header: &carbonado::file::Header) -> u32 {
    header.encoded_len.saturating_sub(header.padding_len)
}

// #[derive(OpenApi)]
// #[openapi(paths(openapi, status))]
// struct ApiDoc;
//
// /// Return JSON version of an OpenAPI schema
// #[utoipa::path(
//     get,
//     path = "/api-docs/openapi.json",
//     responses(
//         (status = 200, description = "JSON file", body = ())
//     )
// )]
// async fn openapi() -> Json<utoipa::openapi::OpenApi> {
//     Json(ApiDoc::openapi())
// }

async fn issue(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(request): Json<IssueRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /issue {request:?}");

    let nostr_hex_sk = auth.token();
    let options = IssueContractOptions {
        proxy: true,
        ..Default::default()
    };
    let issue_res = issue_contract(nostr_hex_sk, request, options).await?;
    Ok((StatusCode::OK, Json(issue_res)))
}

async fn invoice(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(invoice): Json<RgbInvoiceRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /invoice {invoice:?}");

    let nostr_hex_sk = auth.token();
    let options = RgbInvoiceOptions::default();
    let invoice_res = create_invoice(nostr_hex_sk, invoice, options).await?;

    Ok((StatusCode::OK, Json(invoice_res)))
}

async fn transfer(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(req): Json<RgbTransferRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /transfer {req:?}");

    let nostr_hex_sk = auth.token();
    let options = RgbTransferOptions::default();
    let transfer_res = transfer_contract(nostr_hex_sk, req, options).await?;

    Ok((StatusCode::OK, Json(transfer_res)))
}

async fn accept(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(accept_req): Json<AcceptRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /accept {accept_req:?}");

    let nostr_hex_sk = auth.token();
    let options = RgbAcceptOptions::default();
    let accept_res = accept_transfer(nostr_hex_sk, accept_req, options).await?;

    Ok((StatusCode::OK, Json(accept_res)))
}

async fn contracts(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /contracts");

    let nostr_hex_sk = auth.token();
    let contracts_res = list_contracts(nostr_hex_sk, true).await?;
    Ok((StatusCode::OK, Json(contracts_res)))
}

async fn contract_detail(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /contracts/{id:?}");

    let nostr_hex_sk = auth.token();
    let resp = get_contract(nostr_hex_sk, id).await?;
    Ok((StatusCode::OK, Json(resp)))
}

async fn import(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(import_req): Json<String>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /import {import_req:?}");

    let nostr_hex_sk = auth.token();
    let import_res = import_contract(nostr_hex_sk, import_req).await?;

    Ok((StatusCode::OK, Json(import_res)))
}

async fn watcher(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(request): Json<WatcherRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /watcher {request:?}");

    let nostr_hex_sk = auth.token();
    let resp = create_watcher(nostr_hex_sk, request).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn next_address(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(asset): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /watcher/{asset:?}/address");

    let nostr_hex_sk = auth.token();
    let resp = watcher_next_address(nostr_hex_sk, &asset, true).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn next_utxo(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(asset): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /watcher/{asset:?}/utxo");

    let nostr_hex_sk = auth.token();
    let resp = watcher_next_utxo(nostr_hex_sk, &asset).await?;

    Ok((StatusCode::OK, Json(resp)))
}

async fn co_store(
    Path((pk, name)): Path<(String, String)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let cc = CacheControl::new().with_no_cache();

    // v2 headers have no secp256k1 pubkey. `{pk}` is only the on-disk directory
    // (compressed nostr pubkey hex from the client's 32-byte master key).
    let incoming_header = carbonado_header_from_bytes(&body)?;
    let body_len = carbonado_body_len(&incoming_header);
    info!("POST /carbonado/{pk}/{name}, {body_len} bytes");

    let filepath = handle_file(&pk, &name, body_len.try_into()?).await?;

    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&filepath)
    {
        Ok(file) => {
            let present_header = match carbonado_header_from_std_file(&file) {
                Ok(header) => header,
                _ => carbonado_header_from_bytes(&body)?,
            };
            let present_len = carbonado_body_len(&present_header);
            debug!("body len: {body_len} present_len: {present_len}");
            if body_len >= present_len {
                debug!("body is bigger, overwriting.");
                let resp = fs::write(&filepath, &body).await;
                debug!("write file status {}", resp.is_ok());
            } else {
                debug!("no file written.");
            }
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                debug!("no file found, writing {body_len} bytes.");
                fs::write(&filepath, &body).await?;
            }
            _ => {
                error!("error in POST /carbonado/{pk}/{name}: {err}");
                return Err(err.into());
            }
        },
    }

    // metrics::update(&filepath).await?;

    Ok((StatusCode::OK, TypedHeader(cc), "Success"))
}

async fn co_force_store(
    Path((pk, name)): Path<(String, String)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let incoming_header = carbonado_header_from_bytes(&body)?;
    let body_len = carbonado_body_len(&incoming_header);
    info!("POST /carbonado/{pk}/{name}/force, {body_len} bytes");

    let filepath = handle_file(&pk, &name, body_len.try_into()?).await?;

    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&filepath)
    {
        Ok(file) => {
            let present_header = match carbonado_header_from_std_file(&file) {
                Ok(header) => header,
                _ => carbonado_header_from_bytes(&body)?,
            };
            let present_len = carbonado_body_len(&present_header);
            debug!("body len: {body_len} present_len: {present_len}");
            let resp = fs::write(&filepath, &body).await;
            debug!("file override status {}", resp.is_ok());
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                debug!("no file found, writing {body_len} bytes.");
                fs::write(&filepath, &body).await?;
            }
            _ => {
                error!("error in POST /carbonado/{pk}/{name}/force: {err}");
                return Err(err.into());
            }
        },
    }

    let cc = CacheControl::new().with_no_cache();

    Ok((StatusCode::OK, TypedHeader(cc)))
}

async fn co_server_store(
    Path((metadata, name)): Path<(String, String)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /carbonado/server/{name}, {} bytes", body.len());

    let meta = hex::decode(metadata)?;
    let (filepath, encoded) = marketplace_store(&name, &body, Some(meta)).await?;

    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&filepath)
    {
        Ok(file) => {
            let present_header = match carbonado_header_from_std_file(&file) {
                Ok(header) => header,
                Err(_err) => carbonado_header_from_bytes(encoded.as_slice())?,
            };
            let present_len = carbonado_body_len(&present_header);
            fs::write(&filepath, &encoded).await?;

            debug!("present_len: {present_len}");
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                debug!("no file found, writing 0 bytes.");
                fs::write(&filepath, &body).await?;
            }
            _ => {
                error!("error in POST /carbonado/server/{name}: {err}");
                return Err(err.into());
            }
        },
    }

    let cc = CacheControl::new().with_no_cache();
    Ok((StatusCode::OK, TypedHeader(cc)))
}

async fn co_retrieve(
    Path((pk, name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /carbonado/{pk}/{name}");

    let filepath = &handle_file(&pk, &name, 0).await?;
    let fullpath = filepath.to_string_lossy();
    let bytes = fs::read(filepath).await;
    let cc = CacheControl::new().with_no_cache();

    match bytes {
        Ok(bytes) => {
            debug!("read {0} bytes.", bytes.len());
            Ok((StatusCode::OK, TypedHeader(cc), bytes))
        }
        Err(e) => {
            debug!("file read error {0} .Details: {1}.", fullpath, e);
            Ok((StatusCode::OK, TypedHeader(cc), Vec::<u8>::new()))
        }
    }
}

async fn co_metadata(
    Path((pk, name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /carbonado/{pk}/{name}/metadata");

    let filepath = &handle_file(&pk, &name, 0).await?;
    let mut metadata = FileMetadata::default();
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(filepath)
    {
        Ok(file) => {
            let present_header = match carbonado_header_from_std_file(&file) {
                Ok(header) => header,
                _ => return Ok((StatusCode::OK, Json(metadata))),
            };

            metadata.filename = present_header.file_name();
            metadata.metadata = present_header.metadata.unwrap_or_default();
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                fs::write(&filepath, &vec![]).await?;
            }
            _ => {
                error!("error in GET /carbonado/{pk}/{name}/metadata: {err}");
                return Err(err.into());
            }
        },
    }

    Ok((StatusCode::OK, Json(metadata)))
}

async fn co_server_retrieve(Path(name): Path<String>) -> Result<impl IntoResponse, AppError> {
    info!("GET /server/{name}");

    let result = marketplace_retrieve(&name).await;
    let cc = CacheControl::new().with_no_cache();

    match result {
        Ok((bytes, _)) => {
            debug!("read {0} bytes.", bytes.len());
            Ok((StatusCode::OK, TypedHeader(cc), bytes))
        }
        Err(e) => {
            debug!("file read error {0} .Details: {1}.", name, e);
            Ok((StatusCode::OK, TypedHeader(cc), Vec::<u8>::new()))
        }
    }
}

async fn rgb_proxy_consig_save(
    Path(id): Path<String>,
    Json(request): Json<RgbProxyConsigCarbonadoReq>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /proxy/consignment/{id}");
    let request = RgbProxyConsigFileReq::from(request);
    let resp = proxy_consig_store(request).await?;
    Ok((StatusCode::OK, Json(resp)))
}

async fn rgb_proxy_consig_retrieve(Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    info!("GET /proxy/consignment/{id}");

    let id: String = format!("utxob:{id}");
    let resp = proxy_consig_retrieve(&id).await?;
    Ok((StatusCode::OK, Json(resp)))
}

async fn rgb_proxy_media_retrieve(Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    info!("GET /proxy/media/{id}");
    let resp = proxy_media_retrieve(&id).await?;
    Ok((StatusCode::OK, Json(resp)))
}

async fn rgb_proxy_metadata_retrieve(
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /proxy/media-metadata/{id}");
    let resp = proxy_metadata_retrieve(&id).await?;
    Ok((StatusCode::OK, Json(resp)))
}

async fn rgb_proxy_media_data_save(
    Json(request): Json<MediaExtractRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /proxy/media-metadata");

    let MediaExtractRequest {
        encode,
        item: media,
    } = request;
    let resp = proxy_media_data_store(media, encode).await?;
    Ok((StatusCode::OK, Json(resp)))
}

async fn proxy_json_rpc(body: Bytes) -> Result<impl IntoResponse, AppError> {
    info!("-> POST /json-rpc: Request received.");

    // Log the raw request body from the client
    let body_str = String::from_utf8_lossy(&body);
    debug!("   | Received raw body for proxying: {}", body_str);

    let client = reqwest::Client::new();
    let upstream_url = "http://127.0.0.1:3000/json-rpc";

    info!(
        "   | Forwarding request to upstream service at {}",
        upstream_url
    );

    // Send the request to the rgb-proxy service
    let response_result = client
        .post(upstream_url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await;

    // Handle the response from the rgb-proxy service
    let response = match response_result {
        Ok(resp) => {
            info!(
                "   | Received response from upstream with status: {}",
                resp.status()
            );
            resp
        }
        Err(e) => {
            error!("   ! FAILED to connect to upstream service: {}", e);
            // Return a 502 Bad Gateway, which is the correct code for a proxy failure
            return Ok((
                StatusCode::BAD_GATEWAY,
                format!("Failed to connect to upstream service: {}", e),
            )
                .into_response());
        }
    };

    // Read the entire response body as text first
    let response_text = match response.text().await {
        Ok(text) => {
            debug!("   | Received raw response text from upstream: {}", text);
            text
        }
        Err(e) => {
            error!("   ! FAILED to read response body from upstream: {}", e);
            return Ok((
                StatusCode::BAD_GATEWAY,
                format!("Failed to read response from upstream service: {}", e),
            )
                .into_response());
        }
    };

    // Now, try to parse the text as JSON
    let json_response_result = serde_json::from_str::<Value>(&response_text);

    let json_response = match json_response_result {
        Ok(json) => {
            info!("   | Successfully parsed JSON response from upstream.");
            json
        }
        Err(e) => {
            error!(
                "   ! FAILED to parse JSON from upstream response. Error: {}",
                e
            );
            // The body that failed to parse is in the debug log above
            return Ok((
                StatusCode::BAD_GATEWAY,
                format!("Upstream service returned invalid JSON: {}", e),
            )
                .into_response());
        }
    };

    info!("<- POST /json-rpc: Proxying successful. Returning response to client.");
    Ok((StatusCode::OK, Json(json_response)).into_response())
}
/*
async fn rgb_retrieve_swap(Path(name): Path<String>) -> Result<impl IntoResponse, AppError> {
    info!("GET /swap/{name}");

    let result = marketplace_retrieve(&name).await;
    let cc = CacheControl::new().with_no_cache();
    match result {
        Ok((bytes, _)) => {
            debug!("read {0} bytes.", bytes.len());
            Ok((StatusCode::OK, TypedHeader(cc), bytes))
        }
        Err(e) => {
            error!("error in GET /swap/{name}: {e}");
            Ok((StatusCode::OK, TypedHeader(cc), Vec::<u8>::new()))
        }
    }
}

async fn rgb_store_swap(
    Path(name): Path<String>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /swap/{name}");
    let (filepath, encoded) = marketplace_store(&name, &body, None).await?;

    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&filepath)
    {
        Ok(file) => {
            let present_header = match carbonado_header_from_std_file(&file) {
                Ok(header) => header,
                _ => carbonado_header_from_bytes(&body)?,
            };
            let present_len = carbonado_body_len(&present_header);
            debug!("present_len: {present_len}");
            let resp = fs::write(&filepath, &encoded).await;
            debug!("file override status {}", resp.is_ok());
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                debug!("no file found, writing 0 bytes.");
                fs::write(&filepath, &body).await?;
            }
            _ => {
                error!("error in POST /swap/{name}: {err}");
                return Err(err.into());
            }
        },
    }

    let cc = CacheControl::new().with_no_cache();
    Ok((StatusCode::OK, TypedHeader(cc)))
}
*/

async fn rgb_close_auction(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(bundle_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    info!("PUT /auction/{bundle_id}/close");

    let coord_request_key = auth.token();
    let coord_server_key = get_coordinator_nostr_key().await;
    if coord_request_key != coord_server_key {
        return Ok(StatusCode::UNAUTHORIZED);
    }

    match auctions_close(&bundle_id).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(err) => {
            error!("error in PUT /auction/{bundle_id}/close: {err}");
            Err(err.into())
        }
    }
}

async fn coordinator_next_address() -> Result<impl IntoResponse, AppError> {
    info!("GET /coordinator/address");
    let resp = bitmask_core::rgb::coordinator::coordinator_next_address().await?;
    Ok((StatusCode::OK, Json(resp)))
}

async fn coordinator_reset_offers(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /coordinator/reset");

    let coord_request_key = auth.token();
    let coord_server_key = get_coordinator_nostr_key().await;
    if coord_request_key != coord_server_key {
        return Ok(StatusCode::UNAUTHORIZED);
    }

    match reset_public_offers().await {
        Ok(_) => Ok(StatusCode::OK),
        Err(err) => {
            error!("error in GET /coordinator/reset: {err}");
            Err(err.into())
        }
    }
}

async fn coordinator_clean_offers(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /coordinator/clean");

    let coord_request_key = auth.token();
    let coord_server_key = get_coordinator_nostr_key().await;
    if coord_request_key != coord_server_key {
        return Ok(StatusCode::UNAUTHORIZED);
    }

    match clean_public_offers().await {
        Ok(_) => Ok(StatusCode::OK),
        Err(err) => {
            error!("error in GET /coordinator/clean: {err}");
            Err(err.into())
        }
    }
}

async fn rgb_retrieve_auction(
    Path((bundle_id, name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /auction/{bundle_id}/{name}");

    let result = auctions_retrieve(&bundle_id, &name).await;
    let cc = CacheControl::new().with_no_cache();
    match result {
        Ok((bytes, _)) => {
            debug!("read {0} bytes.", bytes.len());
            Ok((StatusCode::OK, TypedHeader(cc), bytes))
        }
        Err(err) => {
            error!("error in GET /auction/{bundle_id}: {err}");
            Ok((StatusCode::OK, TypedHeader(cc), Vec::<u8>::new()))
        }
    }
}

async fn rgb_store_auction(
    Path((bundle_id, metadata, name)): Path<(String, String, String)>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /auction/{bundle_id}/{name}");

    let meta = hex::decode(metadata)?;
    let (filepath, encoded) = auctions_store(&bundle_id, &name, &body, Some(meta)).await?;

    match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&filepath)
    {
        Ok(file) => {
            let present_header = match carbonado_header_from_std_file(&file) {
                Ok(header) => header,
                Err(_err) => carbonado_header_from_bytes(encoded.as_slice())?,
            };
            let present_len = carbonado_body_len(&present_header);
            fs::write(&filepath, &encoded).await?;

            debug!("present_len: {present_len}");
        }
        Err(err) => match err.kind() {
            ErrorKind::NotFound => {
                debug!("no file found, writing 0 bytes.");
                fs::write(&filepath, &body).await?;
            }
            _ => {
                error!("error in POST /auction/{bundle_id}: {err}");
                return Err(err.into());
            }
        },
    }

    let cc = CacheControl::new().with_no_cache();
    Ok((StatusCode::OK, TypedHeader(cc)))
}

async fn status() -> Result<impl IntoResponse, AppError> {
    let cc = CacheControl::new().with_no_cache();

    Ok((StatusCode::OK, TypedHeader(cc), BMC_VERSION.to_string()))
}
/*
async fn key(Path(pk): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sk = env::var("NOSTR_SK")?;
    let sk = SecretKey::from_str(&sk)?;

    let pk = PublicKey::from_str(&pk)?;

    let ss = SharedSecret::new(&pk, &sk);
    let ss = ss.display_secret();

    Ok(ss.to_string())
}
*/

async fn new_block() -> Result<impl IntoResponse, AppError> {
    info!("GET /regtest/block/");
    use bitmask_core::regtest::new_block;
    new_block();

    Ok("Ok")
}

async fn send_coins(
    Path((address, amount)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /regtest/send/{address}/{amount}");

    use bitmask_core::regtest::send_coins;
    send_coins(&address, &amount);

    Ok("Ok")
}

// Mint on server methods

async fn load_rgb_wallets(
    TypedHeader(_auth): TypedHeader<Authorization<Bearer>>,
    Json(request): Json<LoadWalletsRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /loadrgbwallets {request:?}");

    // let _wallets_created = bitmask_core::rgb::load_rgb_wallets(request.number_of_wallets).await?;
    todo!();
    #[allow(unreachable_code)]
    Ok(())
}

async fn atomic_mint(
    TypedHeader(_auth): TypedHeader<Authorization<Bearer>>,
    Json(request): Json<AtomicMintRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("POST /atomic-mint {request:?}");

    bitmask_core::rgb::atomic_refresh().await?;
    let options = IssueContractOptions {
        proxy: true,
        ..Default::default()
    };
    let atomic_mint_res = bitmask_core::rgb::atomic_mint(request, options).await?;
    Ok((StatusCode::OK, Json(atomic_mint_res)))
}

async fn atomic_wallet(
    TypedHeader(_auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /atomic-wallet");

    bitmask_core::rgb::atomic_refresh().await?;

    let atomic_wallet = bitmask_core::rgb::atomic_load().await?;
    Ok((StatusCode::OK, Json(atomic_wallet)))
}

async fn atomic_stats(
    TypedHeader(_auth): TypedHeader<Authorization<Bearer>>,
) -> Result<impl IntoResponse, AppError> {
    info!("GET /atomic-stats");

    bitmask_core::rgb::atomic_refresh().await?;

    let atomic_wallet = bitmask_core::rgb::atomic_stats().await?;
    Ok((StatusCode::OK, Json(atomic_wallet)))
}

async fn json_metrics() -> Result<impl IntoResponse, AppError> {
    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());
    let metrics_json = fs::read_to_string(&format!("{dir}/metrics.json")).await?;

    Ok((
        StatusCode::OK,
        [("content-type", "application/json")],
        metrics_json,
    ))
}

async fn csv_metrics() -> Result<impl IntoResponse, AppError> {
    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());
    let metrics_csv = fs::read_to_string(&format!("{dir}/metrics.csv")).await?;

    Ok((StatusCode::OK, [("content-type", "text/csv")], metrics_csv))
}

async fn init_metrics() -> Result<()> {
    let dir = env::var("CARBONADO_DIR").unwrap_or("/tmp/bitmaskd/carbonado".to_owned());
    let dir = path::Path::new(&dir);
    fs::create_dir_all(dir).await?;

    info!("Starting metrics collection...");
    let duration = Instant::now();

    metrics::init(dir).await?;

    let duration = Instant::now() - duration;

    info!(
        "Finished metrics collection. Took: {} seconds",
        duration.as_secs_f32()
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        unsafe {
            env::set_var("RUST_LOG", "bitmask_core=debug,bitmaskd=debug");
        }
    }

    pretty_env_logger::init();

    let mut app = Router::new()
        .route("/issue", post(issue))
        .route("/invoice", post(invoice))
        .route("/transfer", post(transfer))
        .route("/accept", post(accept))
        .route("/contracts", get(contracts))
        .route("/contracts/:id", get(contract_detail))
        .route("/import", post(import))
        .route("/watcher", post(watcher))
        .route("/watcher/:asset/address", get(next_address))
        .route("/watcher/:asset/utxo", get(next_utxo))
        .route("/json-rpc", post(proxy_json_rpc))
        .route("/proxy/consignment/:id", post(rgb_proxy_consig_save))
        .route("/proxy/consignment/:id", get(rgb_proxy_consig_retrieve))
        .route("/proxy/media-metadata", post(rgb_proxy_media_data_save))
        .route("/proxy/media-metadata/:id", get(rgb_proxy_media_retrieve))
        .route("/proxy/media/:id", get(rgb_proxy_metadata_retrieve))
        // .route("/transfers/check", get(check_transfer))
        // .route("/swap/:name", get(rgb_retrieve_swap))
        // .route("/swap/:name", post(rgb_store_swap))
        .route("/auction/:bundle_id/close", put(rgb_close_auction))
        .route("/auction/:bundle_id/:name", get(rgb_retrieve_auction))
        .route(
            "/auction/:bundle_id/:metadata/:name",
            post(rgb_store_auction),
        )
        .route("/coordinator/address", get(coordinator_next_address))
        .route("/coordinator/reset", get(coordinator_reset_offers))
        .route("/coordinator/clean", get(coordinator_clean_offers))
        .route("/carbonado/status", get(status))
        .route("/carbonado/server/:name", get(co_server_retrieve))
        .route("/carbonado/server/:metadata/:name", post(co_server_store))
        .route("/carbonado/:pk/:name", get(co_retrieve))
        .route("/carbonado/:pk/:name", post(co_store))
        .route("/carbonado/:pk/:name/force", post(co_force_store))
        .route("/carbonado/:pk/:name/metadata", get(co_metadata))
        .route("/loadrgbwallets", post(load_rgb_wallets))
        .route("/atomic-mint", post(atomic_mint))
        .route("/atomic-wallet", get(atomic_wallet))
        .route("/atomic-stats", get(atomic_stats))
        // .route("/key/:pk", get(key))
        .route("/metrics.json", get(json_metrics))
        .route("/metrics.csv", get(csv_metrics));
    // .route("/api-docs/openapi.json", get(openapi))
    // .merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", ApiDoc::openapi()));

    let network = get_network().await;
    if bitmask_core::constants::product_switch_network_supported(&network) {
        switch_network(&network).await?;
    }

    if network == *"regtest" {
        app = app
            .route("/regtest/block", get(new_block))
            .route("/regtest/send/:address/:amount", get(send_coins));
    }

    tokio::spawn(async {
        if let Err(e) = init_metrics().await {
            error!("Error in init metrics: {e}");
        }
    });
    let app = app.layer(DefaultBodyLimit::max(50 * 1024 * 1024));

    let app = app.layer(CorsLayer::permissive());
    let addr = SocketAddr::from(([0, 0, 0, 0], 7070));

    info!("bitmaskd REST server successfully running at {addr}");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// https://github.com/tokio-rs/axum/blob/fef95bf37a138cdf94985e17f27fd36481525171/examples/anyhow-error-response/src/main.rs
// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
