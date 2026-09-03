use anyhow::{Context, Result};
use bech32::{FromBase32, ToBase32, Variant, decode, encode};
#[cfg(not(target_arch = "wasm32"))]
use reqwest::multipart::Form;

#[cfg(target_arch = "wasm32")]
use reqwest::{self, header::AUTHORIZATION};
use serde::Serialize;

#[macro_export]
macro_rules! info {
    ($($arg:expr),+) => {
        let output = [$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::info!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::info!("{}", output);
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:expr),+) => {
        let output = [$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::debug!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::debug!("{}", output);
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:expr),+) => {
        let output = [$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::error!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::error!("{}", output);
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:expr),+) => {
        let output = [$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::warn!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::warn!("{}", output);
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:expr),+) => {
        let output = [$(String::from($arg.to_owned()),)+].join(" ");
        #[cfg(target_arch = "wasm32")]
        gloo_console::trace!(format!("{}", output));
        #[cfg(not(target_arch = "wasm32"))]
        log::trace!("{}", output);
    };
}

#[cfg(target_arch = "wasm32")]
pub async fn post_json<T: Serialize>(url: &str, body: &T) -> Result<(String, u16)> {
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .body(serde_json::to_string(body)?)
        .header("Content-Type", "application/json; charset=UTF-8")
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status().as_u16();

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for POST JSON request to {url}"
    ))?;

    Ok((response_text, status_code))
}

#[cfg(target_arch = "wasm32")]
pub async fn get(url: &str, token: Option<&str>) -> Result<String> {
    let client = reqwest::Client::new();
    let mut response = client.get(url);
    if let Some(t) = token {
        response = response.header(AUTHORIZATION, t);
    }
    let response = response
        .send()
        .await
        .context(format!("Error sending GET request to {url}"))?;

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for GET request to {url}"
    ))?;

    debug!(format!("rgb-proxy.get 9 {response_text}"));
    Ok(response_text)
}

#[cfg(target_arch = "wasm32")]
pub async fn post_json_auth<T: Serialize>(
    url: &str,
    body: &Option<T>,
    token: Option<&str>,
) -> Result<String> {
    let client = reqwest::Client::new();
    let mut response = client.post(url);

    if let Some(b) = body {
        response = response.json(&b);
    }

    if let Some(t) = token {
        response = response.header(AUTHORIZATION, t);
    }

    let response = response
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for POST JSON request to {url}"
    ))?;

    Ok(response_text)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn post_json<T: Serialize>(url: &str, body: &T) -> Result<(String, u16)> {
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .body(serde_json::to_string(body)?)
        .header("Content-Type", "application/json; charset=UTF-8")
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status().as_u16();

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for POST JSON request to {url}"
    ))?;

    Ok((response_text, status_code))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn upload_data(url: &str, form: Form) -> Result<(String, u16)> {
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .multipart(form)
        .header("Content-Type", "multipart/form-data; charset=UTF-8")
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status().as_u16();

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for POST JSON request to {url}"
    ))?;

    debug!(format!("rgb-proxy.post ({status_code}) - {response_text}"));
    Ok((response_text, status_code))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn post_data(url: &str, form: Form) -> Result<(String, u16)> {
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .multipart(form)
        .header("Content-Type", "multipart/form-data; charset=UTF-8")
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let status_code = response.status().as_u16();

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for POST JSON request to {url}"
    ))?;

    debug!(format!("rgb-proxy.get ({status_code}) - {response_text}"));
    Ok((response_text, status_code))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn post_json_auth<T: Serialize>(
    url: &str,
    body: &Option<T>,
    token: Option<&str>,
) -> Result<String> {
    let client = reqwest::Client::new();
    let mut response = client.post(url);

    if let Some(b) = body {
        response = response.json(&b);
    }

    if let Some(t) = token {
        response = response.header(reqwest::header::AUTHORIZATION, t);
    }

    let response = response
        .send()
        .await
        .context(format!("Error sending JSON POST request to {url}"))?;

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for POST JSON request to {url}"
    ))?;

    Ok(response_text)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn get(url: &str, token: Option<&str>) -> Result<String> {
    let client = reqwest::Client::new();
    let mut response = client.get(url);
    if let Some(t) = token {
        response = response.header(reqwest::header::AUTHORIZATION, t);
    }
    let response = response
        .send()
        .await
        .context(format!("Error sending GET request to {url}"))?;

    let response_text = response.text().await.context(format!(
        "Error in parsing server response for GET request to {url}"
    ))?;

    Ok(response_text)
}

pub fn bech32_encode(hrp: &str, bytes: &[u8]) -> Result<String> {
    Ok(encode(hrp, bytes.to_base32(), Variant::Bech32)?)
}

pub fn bech32m_encode(hrp: &str, bytes: &[u8]) -> Result<String> {
    Ok(encode(hrp, bytes.to_base32(), Variant::Bech32m)?)
}

pub const RAW_DATA_ENCODING_DEFLATE: u8 = 1u8;

#[cfg(not(target_arch = "wasm32"))]
pub fn bech32m_zip_encode(hrp: &str, bytes: &[u8]) -> Result<String> {
    use deflate::{Compression, write::DeflateEncoder};
    use std::io::Write;

    // We initialize writer with a version byte, indicating deflation
    // algorithm used
    let writer = vec![RAW_DATA_ENCODING_DEFLATE];
    let mut encoder = DeflateEncoder::new(writer, Compression::Best);
    encoder.write_all(bytes)?;
    let bytes = encoder.finish()?;

    Ok(encode(hrp, bytes.to_base32(), Variant::Bech32m)?)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn bech32m_zip_decode(bech32_str: &str) -> Result<Vec<u8>> {
    use anyhow::anyhow;

    let (_, data, _) = bech32_decode(bech32_str)?;
    match *data[..].first().unwrap_or(&0_u8) {
        RAW_DATA_ENCODING_DEFLATE => {
            let decoded = inflate::inflate_bytes(&data[1..]).map_err(|e| anyhow!(e))?;
            Ok(decoded)
        }
        _ => Err(anyhow!("Unknown version")),
    }
}

pub fn bech32_decode(bech32_str: &str) -> Result<(String, Vec<u8>, Variant)> {
    let (hrp, words, variant) = decode(bech32_str)?;
    Ok((hrp, Vec::<u8>::from_base32(&words)?, variant))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn init_logging(default: &str) {
    use std::env;

    if env::var("RUST_LOG").is_err() {
        // SAFETY: single-threaded test setup before the logger starts.
        unsafe {
            env::set_var("RUST_LOG", format!("bitmask_core=warn,{default}"));
        }
    }

    let _ = pretty_env_logger::formatted_builder()
        .is_test(true)
        .try_init();
}

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
