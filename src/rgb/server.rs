use crate::carbonado::{marketplace_retrieve, marketplace_store};
use crate::constants::deprecated_storage_keys::MARKETPLACE_OFFERS;
use crate::rgb::constants::RGB_DATA_VERSION;
use crate::rgb::models::{OrderId, PublicRgbBid, PublicRgbOffer, RgbOrderbooks};
use postcard::{from_bytes, to_allocvec};
use rgbstd::stl::LIB_ID_RGB_LOGIC as LIB_ID_RGB;

use crate::wallet::server::ServerErrors;
#[cfg(target_arch = "wasm32")]
use amplify::confinement::{Confined, U32};
#[cfg(target_arch = "wasm32")]
use strict_encoding::{StrictDeserialize, StrictSerialize};

#[cfg(target_arch = "wasm32")]
use crate::{
    carbonado::{retrieve, store},
    constants::storage_keys::{ACCOUNT, RGB_STOCK},
    rgb::{
        AccountCrossStore, RGBStock, RgbAccount,
        data::{RGBStockBlob, RgbAccountBlob},
    },
};

// TODO: Change JSON to more fast encode/decode format
pub async fn get_public_offer(offer_id: OrderId) -> Result<Option<PublicRgbOffer>, ServerErrors> {
    let name = MARKETPLACE_OFFERS;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(none!())
    } else {
        let data: String = from_bytes(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let orderbooks: RgbOrderbooks = serde_json::from_str(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let mut all_offers = vec![];
        for offers in orderbooks.offers.values() {
            let offers = offers.to_vec();
            all_offers.extend(offers.iter().filter(|x| x.is_open()).map(|x| x.to_owned()));
        }

        Ok(all_offers.iter().find(|x| x.id == offer_id).cloned())
    }
}

// TODO: Change JSON to more fast encode/decode format
pub async fn remove_public_offer(
    offer_id: OrderId,
) -> Result<Option<PublicRgbOffer>, ServerErrors> {
    let name = MARKETPLACE_OFFERS;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(none!())
    } else {
        let data: String = from_bytes(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let mut orderbooks: RgbOrderbooks = serde_json::from_str(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let mut all_offers = vec![];
        for offers in orderbooks.offers.values() {
            let offers = offers.to_vec();
            all_offers.extend(offers.iter().filter(|x| x.is_open()).map(|x| x.to_owned()));
        }

        if let Some(public_offer) = all_offers.iter().find(|x| x.id == offer_id)
            && let Some(offers) = orderbooks.offers.get(&public_offer.contract_id())
            && let Some(position) = offers.iter().position(|x| x.id == offer_id && x.is_open())
        {
            let mut public_offers = offers.clone();
            let mut public_offer = public_offers.remove(position);
            public_offer.cancel();

            public_offers.insert(position, public_offer.clone());

            orderbooks
                .offers
                .insert(public_offer.contract_id(), public_offers.clone());

            let content = serde_json::to_string(&orderbooks)
                .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

            let data = to_allocvec(&content)
                .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

            marketplace_store(
                &format!("{hashed_name}.c15"),
                &data,
                Some(RGB_DATA_VERSION.to_vec()),
            )
            .await
            .map_err(|op| ServerErrors::CarbonadoWrite(name.to_string(), op.to_string()))?;

            return Ok(Some(public_offer));
        }

        Ok(none!())
    }
}

// TODO: Change JSON to more fast encode/decode format
pub async fn remove_public_bid(
    offer_id: OrderId,
    bid_id: OrderId,
) -> Result<Option<PublicRgbBid>, ServerErrors> {
    let name = MARKETPLACE_OFFERS;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(none!())
    } else {
        let data: String = from_bytes(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let mut orderbooks: RgbOrderbooks = serde_json::from_str(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let mut all_bids = vec![];
        if let Some(bids) = orderbooks.bids.get(&offer_id) {
            let bids: Vec<_> = bids.values().cloned().collect();
            all_bids.extend(bids.iter().filter(|x| x.is_open()).map(|x| x.to_owned()));
        }

        if let Some(public_bid) = all_bids.iter().find(|x| x.id == bid_id)
            && let Some(mut bid) = orderbooks.bids.remove(&public_bid.id)
        {
            let content = serde_json::to_string(&orderbooks)
                .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

            let data = to_allocvec(&content)
                .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

            marketplace_store(
                &format!("{hashed_name}.c15"),
                &data,
                Some(RGB_DATA_VERSION.to_vec()),
            )
            .await
            .map_err(|op| ServerErrors::CarbonadoWrite(name.to_string(), op.to_string()))?;

            if let Some((_, bid)) = bid.pop_first() {
                return Ok(Some(bid));
            }
        }
        Ok(none!())
    }
}

pub async fn list_public_offers() -> Result<Vec<PublicRgbOffer>, ServerErrors> {
    let name = MARKETPLACE_OFFERS;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(vec![])
    } else {
        let data: String = from_bytes(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let orderbooks: RgbOrderbooks = serde_json::from_str(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let mut all_offers = vec![];
        for offers in orderbooks.offers.values() {
            let offers = offers.to_vec();

            all_offers.extend(offers.iter().filter(|x| x.is_open()).map(|x| x.to_owned()));
        }

        Ok(all_offers)
    }
}

pub async fn list_fill_public_offers() -> Result<Vec<PublicRgbOffer>, ServerErrors> {
    let name = MARKETPLACE_OFFERS;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(vec![])
    } else {
        let data: String = from_bytes(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let orderbooks: RgbOrderbooks = serde_json::from_str(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let mut all_offers = vec![];
        for offers in orderbooks.offers.values() {
            let offers = offers.to_vec();

            all_offers.extend(offers.iter().filter(|x| x.is_fill()).map(|x| x.to_owned()));
        }

        Ok(all_offers)
    }
}

// TODO: Change JSON to more fast encode/decode format
pub async fn save_public_offers(offers: Vec<PublicRgbOffer>) -> Result<(), ServerErrors> {
    let name = MARKETPLACE_OFFERS;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    let mut public_books: RgbOrderbooks = if data.is_empty() {
        RgbOrderbooks::new()
    } else {
        let content = from_bytes(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        serde_json::from_str(content)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?
    };

    for offer in offers {
        public_books
            .offers
            .entry(offer.contract_id())
            .and_modify(|x| {
                x.push(offer.clone());
            })
            .or_insert(vec![offer.clone()]);
    }

    let content = serde_json::to_string(&public_books)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    let data = to_allocvec(&content)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    marketplace_store(
        &format!("{hashed_name}.c15"),
        &data,
        Some(RGB_DATA_VERSION.to_vec()),
    )
    .await
    .map_err(|op| ServerErrors::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

/// Cancel every open marketplace offer. Filled offers stay.
pub async fn reset_public_offers() -> Result<(), ServerErrors> {
    let name = MARKETPLACE_OFFERS;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        return Ok(());
    }

    let data: String =
        from_bytes(&data).map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

    let mut orderbooks: RgbOrderbooks = serde_json::from_str(&data)
        .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

    orderbooks.reset_open_offers();

    let content = serde_json::to_string(&orderbooks)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    let data = to_allocvec(&content)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    marketplace_store(
        &format!("{hashed_name}.c15"),
        &data,
        Some(RGB_DATA_VERSION.to_vec()),
    )
    .await
    .map_err(|op| ServerErrors::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

/// Remove filled marketplace offers.
pub async fn clean_public_offers() -> Result<(), ServerErrors> {
    let name = MARKETPLACE_OFFERS;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        return Ok(());
    }

    let data: String =
        from_bytes(&data).map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

    let mut orderbooks: RgbOrderbooks = serde_json::from_str(&data)
        .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

    orderbooks.clean_fill_offers();

    let content = serde_json::to_string(&orderbooks)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    let data = to_allocvec(&content)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    marketplace_store(
        &format!("{hashed_name}.c15"),
        &data,
        Some(RGB_DATA_VERSION.to_vec()),
    )
    .await
    .map_err(|op| ServerErrors::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

pub async fn match_public_offer(mut public_bid: PublicRgbBid) -> Result<(), ServerErrors> {
    let name = MARKETPLACE_OFFERS;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _) = marketplace_retrieve(main_name)
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    let mut public_books: RgbOrderbooks = if data.is_empty() {
        RgbOrderbooks::new()
    } else {
        let content = from_bytes(&data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        serde_json::from_str(content)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?
    };

    if let Some(contract_id) = public_bid.counter_party()
        && let Some(offers) = public_books.offers.get(&contract_id)
        && let Some(position) = offers
            .iter()
            .position(|x| x.id == public_bid.offer_id && x.is_open())
    {
        let mut public_offers = offers.clone();
        let mut public_offer = public_offers.remove(position);

        public_offer.swap_suppl = public_bid.swap.clone();
        public_bid.complete();
        public_offer.complete();

        let mut slot = bmap! {};
        slot.insert(public_bid.id, public_bid.clone());
        public_offers.insert(position, public_offer.clone());

        public_books
            .offers
            .insert(contract_id, public_offers.clone());
        public_books.bids.insert(public_offer.id, slot);
    }

    let content = serde_json::to_string(&public_books)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    let data = to_allocvec(&content)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    marketplace_store(
        &format!("{hashed_name}.c15"),
        &data,
        Some(RGB_DATA_VERSION.to_vec()),
    )
    .await
    .map_err(|op| ServerErrors::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub async fn backup_rgb_stock(sk: &str, rgb_stock: RGBStock) -> Result<(), ServerErrors> {
    let name = RGB_STOCK;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let dump = RGBStockBlob::dump(rgb_stock);
    let data = dump
        .to_strict_serialized::<U32>()
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    store(sk, main_name, &data, false, Some(RGB_DATA_VERSION.to_vec()))
        .await
        .map_err(|op| ServerErrors::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub async fn restore_rgb_stock(sk: &str) -> Result<RGBStock, ServerErrors> {
    let name = RGB_STOCK;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _metadata) = retrieve(sk, main_name, vec![])
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        Ok(RGBStock::in_memory())
    } else {
        let confined = Confined::try_from_iter(data)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let RGBStockBlob {
            stash,
            state,
            index,
        } = RGBStockBlob::from_strict_serialized::<U32>(confined)
            .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

        let stock = RGBStock::with(stash, state, index);
        Ok(stock)
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn backup_rgb_account(
    sk: &str,
    provider: &mut AccountCrossStore,
) -> Result<(), ServerErrors> {
    let name = ACCOUNT;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");

    let dump: RgbAccountBlob = provider.rgb_backup().await.expect("invalid dump");
    let content = serde_json::to_string(&dump)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    let data = to_allocvec(&content)
        .map_err(|op| ServerErrors::Encode(name.to_string(), op.to_string()))?;

    store(sk, main_name, &data, false, Some(RGB_DATA_VERSION.to_vec()))
        .await
        .map_err(|op| ServerErrors::CarbonadoWrite(name.to_string(), op.to_string()))?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub async fn restore_rgb_account(
    sk: &str,
    provider: &mut AccountCrossStore,
) -> Result<RgbAccount, ServerErrors> {
    let name = ACCOUNT;
    let hashed_name = blake3::hash(format!("{LIB_ID_RGB}-{name}").as_bytes())
        .to_hex()
        .to_ascii_lowercase();

    let main_name = &format!("{hashed_name}.c15");
    let (data, _metadata) = retrieve(sk, main_name, vec![])
        .await
        .map_err(|op| ServerErrors::CarbonadoRetrieve(name.to_string(), op.to_string()))?;

    if data.is_empty() {
        return Err(ServerErrors::NoFile(name.to_string()));
    }
    let data: String =
        from_bytes(&data).map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

    let blob: RgbAccountBlob = serde_json::from_str(&data)
        .map_err(|op| ServerErrors::Decode(name.to_string(), op.to_string()))?;

    let rgb_account = provider.rgb_restore(blob).await.expect("invalid backup");
    Ok(rgb_account)
}
