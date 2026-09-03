#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use bitmask_core::{
    carbonado::{auctions_retrieve, auctions_store, error::CarbonadoError},
    constants::get_coordinator_nostr_key,
};
use carbonado::file::Header;

/// Carbonado 0.7 auction objects use the coordinator 32-byte nostr key.
/// RGB 0.10 ECDH SharedSecret (bundle pubkey × coordinator sk) is gone.
#[tokio::test]
async fn auctions_store_retrieve_roundtrip_carbonado_v2_coordinator_key() -> Result<()> {
    let bundle_id = "auction-roundtrip-bundle";
    let name = "offers.c15";
    let payload = b"rgb-011-auction-carbonado-v2";
    let meta = Some(vec![0x11, 0x22]);

    let (path, encoded) = auctions_store(bundle_id, name, payload, meta).await?;
    assert!(
        encoded.len() > Header::LEN,
        "auction object is a Carbonado v2 file"
    );
    let header = Header::try_from(&encoded[..Header::LEN])?;
    assert_eq!(header.try_to_vec()?.len(), Header::LEN);

    let sk = hex::decode(get_coordinator_nostr_key().await)?;
    assert_eq!(sk.len(), 32, "coordinator master key is 32 bytes");
    let (_decoded_header, decoded) = carbonado::file::decode(&sk, &encoded)?;
    assert_eq!(decoded, payload, "coordinator key decodes; not ECDH");

    let on_disk = tokio::fs::read(&path).await?;
    assert_eq!(on_disk, encoded);

    let (retrieved, _) = auctions_retrieve(bundle_id, name).await?;
    assert_eq!(retrieved, payload);
    Ok(())
}

#[tokio::test]
async fn auctions_retrieve_missing_bundle_is_empty() -> Result<()> {
    let (data, meta) = auctions_retrieve("auction-missing-bundle", "none.c15").await?;
    assert!(data.is_empty());
    assert!(meta.is_none());
    Ok(())
}

#[tokio::test]
async fn auctions_store_rejects_path_bundle_id() -> Result<()> {
    let err = auctions_store("../escape", "offers.c15", b"nope", None)
        .await
        .expect_err("path bundle id is rejected");
    assert!(matches!(err, CarbonadoError::InvalidAuctionBundleId));

    let err = auctions_store("a/b", "offers.c15", b"nope", None)
        .await
        .expect_err("slash bundle id is rejected");
    assert!(matches!(err, CarbonadoError::InvalidAuctionBundleId));
    Ok(())
}

#[tokio::test]
async fn auctions_store_rejects_path_name() -> Result<()> {
    let err = auctions_store("bundle", "../escape.c15", b"nope", None)
        .await
        .expect_err("path name is rejected");
    assert!(matches!(err, CarbonadoError::InvalidAuctionBundleId));

    let err = auctions_store("bundle", "a/b", b"nope", None)
        .await
        .expect_err("slash name is rejected");
    assert!(matches!(err, CarbonadoError::InvalidAuctionBundleId));
    Ok(())
}
