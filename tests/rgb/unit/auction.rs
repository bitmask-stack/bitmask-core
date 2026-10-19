#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use bitmask_core::{
    carbonado::{
        AUCTION_CLOSED_NAME, auctions_close, auctions_retrieve, auctions_store,
        error::CarbonadoError,
    },
    constants::{CoordinatorError, get_coordinator_nostr_key, get_coordinator_xpub},
    rgb::{
        coordinator::coordinator_next_address,
        models::{
            ContractAmount, ContractInfo, ContractKit, CounterParty, OrderId, PublicRgbOffer,
            RgbOrderbooks, Status,
        },
    },
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

/// Close writes a Carbonado v2 marker under the bundle. Not RGB 0.10 price matching.
#[tokio::test]
async fn auctions_close_roundtrip_carbonado_v2_coordinator_key() -> Result<()> {
    let bundle_id = "auction-close-roundtrip-bundle";
    let name = "offers.c15";
    let payload = b"rgb-011-auction-offer";

    auctions_store(bundle_id, name, payload, None).await?;

    let (_path, encoded) = auctions_close(bundle_id).await?;
    assert!(
        encoded.len() > Header::LEN,
        "closed marker is a Carbonado v2 file"
    );
    let header = Header::try_from(&encoded[..Header::LEN])?;
    assert_eq!(header.try_to_vec()?.len(), Header::LEN);

    let sk = hex::decode(get_coordinator_nostr_key().await)?;
    assert_eq!(sk.len(), 32, "coordinator master key is 32 bytes");
    let (_decoded_header, decoded) = carbonado::file::decode(&sk, &encoded)?;
    assert_eq!(decoded, b"closed");

    let (marker, _) = auctions_retrieve(bundle_id, AUCTION_CLOSED_NAME).await?;
    assert!(!marker.is_empty(), "closed marker retrieve is non-empty");
    assert_eq!(marker, b"closed");

    let (retrieved, _) = auctions_retrieve(bundle_id, name).await?;
    assert_eq!(retrieved, payload);
    Ok(())
}

#[tokio::test]
async fn auctions_close_rejects_path_bundle_id() -> Result<()> {
    let err = auctions_close("../escape")
        .await
        .expect_err("path bundle id is rejected");
    assert!(matches!(err, CarbonadoError::InvalidAuctionBundleId));

    let err = auctions_close("a/b")
        .await
        .expect_err("slash bundle id is rejected");
    assert!(matches!(err, CarbonadoError::InvalidAuctionBundleId));
    Ok(())
}

#[tokio::test]
async fn auctions_store_after_close_fails() -> Result<()> {
    let bundle_id = "auction-store-after-close-bundle";
    auctions_store(bundle_id, "offers.c15", b"open", None).await?;
    auctions_close(bundle_id).await?;

    let err = auctions_store(bundle_id, "late.c15", b"too-late", None)
        .await
        .expect_err("store after close is rejected");
    assert!(matches!(err, CarbonadoError::AuctionBundleClosed));
    Ok(())
}

fn fixture_public_offer(id_byte: u8, status: Status) -> PublicRgbOffer {
    PublicRgbOffer {
        id: OrderId::from([id_byte; 32]),
        status,
        amount: ContractAmount::new(1, 0, 0),
        asset: ContractKit::new(ContractInfo::default(), String::new()),
        counter_party: CounterParty::Sats(1),
        order_suppl: None,
        swap_suppl: None,
    }
}

#[test]
fn orderbooks_reset_empties_open_and_clean_drops_fill() {
    let open = fixture_public_offer(1, Status::Open);
    let fill = fixture_public_offer(2, Status::Finish);
    let contract_id = open.contract_id();

    let mut books = RgbOrderbooks::new();
    books
        .offers
        .insert(contract_id, vec![open.clone(), fill.clone()]);

    books.reset_open_offers();
    let remaining: Vec<_> = books.offers.values().flatten().cloned().collect();
    assert!(
        remaining.iter().all(|offer| !offer.is_open()),
        "reset clears open offers"
    );
    assert!(
        remaining.iter().any(|offer| offer.is_fill()),
        "reset keeps filled offers"
    );

    books.clean_fill_offers();
    let remaining: Vec<_> = books.offers.values().flatten().cloned().collect();
    assert!(
        remaining.iter().all(|offer| !offer.is_fill()),
        "clean drops filled offers"
    );
}

#[tokio::test]
async fn coordinator_next_address_empty_xpub_is_typed_error() -> Result<()> {
    let xpub = get_coordinator_xpub().await;
    assert!(xpub.is_empty(), "fixture COORDINATOR_XPUB is empty");
    let err = coordinator_next_address()
        .await
        .expect_err("empty coordinator xpub is a typed error");
    assert!(matches!(err, CoordinatorError::EmptyXpub));
    Ok(())
}
