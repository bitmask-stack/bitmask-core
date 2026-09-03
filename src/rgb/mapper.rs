use crate::rgb::models::{
    AssetKit, BidAsset, ContractKit, ContractMeta, ContractSwap, CounterParty, PublicRgbBid,
    PublicRgbOffer, RgbBid, RgbOffer, UtxoKit,
};
use crate::rgb::proxy::{
    ContractMedia, ContractMediaItem, MediaItem, MediaMetadata, MediaPublished,
    RgbProxyConsigCarbonadoReq, RgbProxyConsigFileReq,
};
use crate::structs::{
    ContractCounterParty, IssueMedia, MediaData, OrderValue, RgbBidResponse, RgbOfferResponse,
    TxStatus, UDAValue as StructAllocation,
};
use amplify::hex::ToHex;
use bpwallet::WalletUtxo;
use rgbcore::{SchemaId, StateType};
use rgbinvoice::{Allocation, OwnedFraction};
use rgbstd::contract::{AllocatedState, IssuerWrapper};
use rgbstd::stl::EmbeddedMedia;
use rgbstd::{Allocation as SchemaAllocation, stl::Attachment};
use schemata::{
    CFA_SCHEMA_ID, CollectibleFungibleAsset, IFA_SCHEMA_ID, InflatableFungibleAsset, NIA_SCHEMA_ID,
    NonInflatableAsset, PFA_SCHEMA_ID, PermissionedFungibleAsset, UDA_SCHEMA_ID,
    UniqueDigitalAsset,
};
use std::fmt;
use std::fmt::{Display, Formatter};
use strict_types::TypeSystem;

impl From<EmbeddedMedia> for MediaData {
    fn from(value: EmbeddedMedia) -> Self {
        let EmbeddedMedia { ty, data } = value;

        Self {
            ty: ty.to_string(),
            uri: base64::encode(&data),
        }
    }
}

impl From<Attachment> for MediaData {
    fn from(value: Attachment) -> Self {
        let Attachment { ty, digest } = value;
        Self {
            ty: ty.to_string(),
            uri: digest.to_hex(),
        }
    }
}

impl From<MediaMetadata> for MediaData {
    fn from(value: MediaMetadata) -> Self {
        let MediaMetadata { mime: ty, uri, .. } = value;
        Self { ty, uri }
    }
}

impl From<MediaData> for ContractMediaItem {
    fn from(value: MediaData) -> Self {
        let MediaData { ty, uri } = value;
        Self { ty, source: uri }
    }
}

impl From<MediaItem> for ContractMediaItem {
    fn from(value: MediaItem) -> Self {
        let MediaItem { ty, source, .. } = value;
        Self { ty, source }
    }
}

impl From<IssueMedia> for ContractMedia {
    fn from(value: IssueMedia) -> Self {
        let IssueMedia {
            preview,
            media,
            attachments,
        } = value;

        Self {
            preview: preview.map(ContractMediaItem::from),
            media: media.map(ContractMediaItem::from),
            attachments: attachments
                .into_iter()
                .map(ContractMediaItem::from)
                .collect(),
        }
    }
}

impl From<MediaPublished> for ContractMedia {
    fn from(value: MediaPublished) -> Self {
        let MediaPublished {
            preview,
            media,
            attachments,
        } = value;

        Self {
            preview: preview.map(ContractMediaItem::from),
            media: media.map(ContractMediaItem::from),
            attachments: attachments
                .into_iter()
                .map(ContractMediaItem::from)
                .collect(),
        }
    }
}

impl From<SchemaAllocation> for StructAllocation {
    fn from(value: SchemaAllocation) -> Self {
        Self {
            token_index: value.token_index().into(),
            fraction: value.fraction().into(),
        }
    }
}

#[allow(clippy::expect_used)]
impl From<RgbProxyConsigCarbonadoReq> for RgbProxyConsigFileReq {
    fn from(value: RgbProxyConsigCarbonadoReq) -> Self {
        let RgbProxyConsigCarbonadoReq {
            params,
            file_name,
            hex,
        } = value;

        Self {
            params,
            file_name,
            bytes: hex::decode(hex).expect("Error when parse hexadecimal data"),
        }
    }
}

impl From<RgbProxyConsigFileReq> for RgbProxyConsigCarbonadoReq {
    fn from(value: RgbProxyConsigFileReq) -> Self {
        let RgbProxyConsigFileReq {
            params,
            file_name,
            bytes,
        } = value;

        Self {
            params,
            file_name,
            hex: hex::encode(bytes),
        }
    }
}

impl From<bpwallet::TxStatus> for TxStatus {
    fn from(value: bpwallet::TxStatus) -> Self {
        match value {
            bpwallet::TxStatus::Unknown => TxStatus::Error("unknown transaction".to_string()),
            bpwallet::TxStatus::Channel | bpwallet::TxStatus::Mempool => TxStatus::Mempool,
            bpwallet::TxStatus::Mined(info) => TxStatus::Block {
                height: info.height.into(),
                timestamp: info.time,
            },
        }
    }
}

impl From<WalletUtxo> for UtxoKit {
    fn from(value: WalletUtxo) -> Self {
        let WalletUtxo {
            outpoint,
            status,
            terminal,
            value,
            ..
        } = value;

        Self {
            outpoint: crate::rgb::compat::outpoint_bp_to_bitcoin(outpoint),
            value,
            terminal,
            status,
        }
    }
}
impl From<ContractMeta> for ContractKit {
    fn from(value: ContractMeta) -> Self {
        Self::new(value.info().clone(), String::new())
    }
}

impl From<BidAsset> for AssetKit {
    fn from(value: BidAsset) -> Self {
        match &value {
            BidAsset::L1(_) => AssetKit::Bitcoin,
            BidAsset::Contract(contract) => AssetKit::Contract(contract.clone().into()),
        }
    }
}

impl From<PublicRgbOffer> for RgbOfferResponse {
    fn from(value: PublicRgbOffer) -> Self {
        let PublicRgbOffer {
            id,
            status,
            asset: contract,
            counter_party,
            amount: contract_amount,
            ..
        } = value.clone();

        Self {
            id: id.to_string(),
            status: status.to_string(),
            amount: contract_amount,
            counter_party: counter_party.into(),
            contract,
        }
    }
}

impl From<RgbOffer> for RgbOfferResponse {
    fn from(value: RgbOffer) -> Self {
        Self {
            id: value.id().to_string(),
            status: value.status().to_string(),
            amount: *value.amount(),
            counter_party: value.counter_party().clone().into(),
            contract: ContractKit::new(value.asset().info().clone(), String::new()),
        }
    }
}

impl From<PublicRgbBid> for RgbBidResponse {
    fn from(value: PublicRgbBid) -> Self {
        let PublicRgbBid {
            id,
            offer_id,
            status,
            asset,
            amount: contract_amount,
            counter_party,
            ..
        } = value.clone();

        Self {
            id: id.to_string(),
            offer_id: offer_id.to_string(),
            status: status.to_string(),
            amount: contract_amount,
            counter_party: counter_party.into(),
            asset,
            txid: String::new(),
        }
    }
}

impl From<RgbBid> for RgbBidResponse {
    fn from(value: RgbBid) -> Self {
        Self {
            id: value.id().to_string(),
            offer_id: value.offer_id().to_string(),
            status: value.status().to_string(),
            amount: *value.amount(),
            counter_party: value.counter_party().clone().into(),
            asset: value.asset().clone().into(),
            txid: String::new(),
        }
    }
}

impl From<CounterParty> for OrderValue {
    fn from(value: CounterParty) -> Self {
        match value {
            CounterParty::Sats(sats) => OrderValue::Bitcoin(sats),
            CounterParty::Contract(cp) => {
                OrderValue::Contract(ContractCounterParty::with(cp.kit, cp.amount))
            }
        }
    }
}

impl From<OrderValue> for CounterParty {
    fn from(value: OrderValue) -> Self {
        match value {
            OrderValue::Bitcoin(sats) => CounterParty::Sats(sats),
            OrderValue::Contract(cp) => {
                CounterParty::Contract(ContractSwap::with(cp.kit, cp.amount))
            }
        }
    }
}

pub const UDA_FIXED_INDEX: u32 = 0;
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AssetSchema {
    Nia,
    Uda,
    Cfa,
    Pfa,
    Ifa,
}

impl Display for AssetSchema {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", format!("{self:?}").to_lowercase())
    }
}

impl AssetSchema {
    pub fn types(&self) -> TypeSystem {
        match self {
            Self::Nia => NonInflatableAsset::types(),
            Self::Uda => UniqueDigitalAsset::types(),
            Self::Cfa => CollectibleFungibleAsset::types(),
            Self::Pfa => PermissionedFungibleAsset::types(),
            Self::Ifa => InflatableFungibleAsset::types(),
        }
    }

    pub fn default_state_type(&self) -> StateType {
        match self {
            Self::Cfa | Self::Nia | Self::Pfa | Self::Ifa => StateType::Fungible,
            Self::Uda => StateType::Structured,
        }
    }

    pub fn allocated_state(&self, value: u64) -> AllocatedState {
        match self {
            Self::Cfa | Self::Nia | Self::Pfa | Self::Ifa => AllocatedState::Amount(value.into()),
            Self::Uda => AllocatedState::Data(
                Allocation::with(UDA_FIXED_INDEX, OwnedFraction::from(1)).into(),
            ),
        }
    }
}

impl From<SchemaId> for AssetSchema {
    fn from(schema_id: SchemaId) -> Self {
        match schema_id {
            CFA_SCHEMA_ID => AssetSchema::Cfa,
            NIA_SCHEMA_ID => AssetSchema::Nia,
            UDA_SCHEMA_ID => AssetSchema::Uda,
            PFA_SCHEMA_ID => AssetSchema::Pfa,
            IFA_SCHEMA_ID => AssetSchema::Ifa,
            _ => panic!("unknown schema ID"),
        }
    }
}
