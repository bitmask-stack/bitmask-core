use std::str::FromStr;

use crate::rgb::mapper::AssetSchema;
use crate::rgb::proxy::ContractMedia;
use crate::rgb::{models::AssetType, models::ContractAmount};
use crate::structs::{IssueRequest, SecretString};
use amplify::confinement::{Confined, SmallBlob};
use bpstd::Outpoint;
use bpwallet::OutpointParseError;
use dbc::Method;
use rgb::{
    Allocation, GenesisSeal, TokenIndex,
    validation::{Failure, ResolveWitness},
};
use rgbcore::{ChainNet, Identity};
use rgbstd::containers::ConsignmentExt;
use rgbstd::contract::{BuilderError, ContractBuilder, IssuerWrapper};
use rgbstd::stl::{AssetSpec, ContractTerms, EmbeddedMedia, TokenData};
use rgbstd::{
    OwnedFraction,
    containers::{BuilderSeal, Contract},
    invoice::{Amount, Precision},
    persistence::Stock,
    stl::{Attachment, MediaType, RicardianContract},
};
use schemata::{NIA_SCHEMA_ID, NonInflatableAsset, UDA_SCHEMA_ID, UniqueDigitalAsset};
use strict_encoding::{TypeName, tn};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum IssueContractError {
    /// The contract interface {0} is not supported
    NoSupport(String),
    /// contract media {0} cannot be parsed. {1}
    InvalidMedia(String, String),
    /// preview media exceded max number of bytes. expected: 65535 / current: {0}
    WrongPreview(usize),
    /// The contract {0} contains failures {1:?}
    WrongContract(String, Box<Vec<Failure>>),
    /// The contract {0} cannot be imported. {1}
    WrongImport(String, String),
    /// rgb stock error. {0}
    Stock(String),
    /// rgb seals txout error. {0}
    SealsTxoutError(#[from] OutpointParseError),
    /// rgb strict encoding error - u8. {0}
    StenErrorU8(#[from] strict_encoding::VariantError<u8>),
    /// rgb strict encoding error - r sring. {0}
    StenErrorRStr(#[from] strict_encoding::InvalidRString),
    /// rgb interface builder error. {0}
    RgbInterfaceBuilderError(#[from] BuilderError),
}

#[derive(Clone, Debug, Display, Error, From)]
#[display(doc_comments)]
pub struct IssueContractParams {
    pub ticker: String,
    pub name: String,
    pub description: String,
    pub precision: u8,
    pub supply: u64,
    pub iface: TypeName,
    pub seal: SecretString,
    pub chain_net: ChainNet,
    pub meta: Option<ContractMedia>,
}

impl IssueContractParams {
    pub fn with(value: IssueRequest, chain_net: ChainNet, media: Option<ContractMedia>) -> Self {
        let IssueRequest {
            ticker,
            name,
            description,
            precision,
            supply,
            iface,
            seal,
            ..
        } = value;

        let contract_amount = ContractAmount::parse(supply.to_string(), precision);
        Self {
            ticker,
            name,
            description,
            precision,
            chain_net,
            meta: media,
            iface: tn!(iface),
            seal: SecretString(seal),
            supply: contract_amount.to_value(),
        }
    }
}

#[derive(Clone)]
/// Options used in issuing asset operation
pub struct IssueContractOptions {
    /// Define the default close method for issue assets
    pub method: Method,
    /// Feature-flag to enable send data to proxy server
    pub proxy: bool,
    /// Feature-flag to enable sync data to carbonado server
    pub sync: bool,
}

impl Default for IssueContractOptions {
    fn default() -> Self {
        Self {
            method: Method::TapretFirst,
            sync: true,
            proxy: false,
        }
    }
}

#[allow(clippy::result_large_err)]
pub fn issue_contract<T>(
    params: IssueContractParams,
    options: &IssueContractOptions,
    rgb_stock: &mut Stock,
    rgb_resolver: &mut T,
) -> Result<Contract, IssueContractError>
where
    T: ResolveWitness + Clone,
{
    let IssueContractParams {
        iface, chain_net, ..
    } = params.clone();

    let iface = iface.to_string();
    let asset_type =
        AssetType::from_str(&iface).map_err(|_| IssueContractError::NoSupport(iface))?;

    let contract_issued = match asset_type {
        AssetType::RGB20 => issue_fungible_asset(params, options)?,
        AssetType::RGB21 => issue_uda_asset(params, options)?,
        err => return Err(IssueContractError::NoSupport(err.to_string())),
    };

    let contract_id = contract_issued.contract_id();

    let type_system = match asset_type {
        AssetType::RGB20 => AssetSchema::from(NIA_SCHEMA_ID).types(),
        AssetType::RGB21 => AssetSchema::from(UDA_SCHEMA_ID).types(),
        _ => return Err(IssueContractError::NoSupport(asset_type.to_string())),
    };

    let valid_contract = match contract_issued.validate(
        rgb_resolver,
        &crate::rgb::compat::validation_config(chain_net, type_system),
    ) {
        Ok(contract) => contract,
        Err(err) => {
            let failures = match err {
                rgb::validation::ValidationError::InvalidConsignment(failure) => {
                    vec![failure]
                }
                other => vec![rgb::validation::Failure::Custom(other.to_string())],
            };
            return Err(IssueContractError::WrongContract(
                contract_id.to_string(),
                Box::new(failures),
            ));
        }
    };

    rgb_stock
        .import_contract(valid_contract, rgb_resolver.clone())
        .map_err(|err| IssueContractError::WrongImport(contract_id.to_string(), err.to_string()))?;

    let final_contract = rgb_stock
        .export_contract(contract_id)
        .map_err(|err| IssueContractError::WrongImport(contract_id.to_string(), err.to_string()))?;
    Ok(final_contract)
}

#[allow(clippy::result_large_err)]
fn issue_fungible_asset(
    params: IssueContractParams,
    _options: &IssueContractOptions,
) -> Result<Contract, IssueContractError> {
    let IssueContractParams {
        ticker,
        name,
        description,
        precision,
        seal,
        supply,
        chain_net,
        ..
    } = params;
    let ticker: &'static str = Box::leak(ticker.to_string().into_boxed_str());
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());

    let precision = Precision::try_from(precision)?;
    let spec = AssetSpec::new(ticker, name, precision);
    let text = RicardianContract::from_str(&description)?;
    let contract_data = ContractTerms { text, media: None };

    let seal =
        Outpoint::from_str(&seal.to_string()).map_err(IssueContractError::SealsTxoutError)?;

    let seal = GenesisSeal::new_random(
        crate::rgb::compat::txid_bp_to_bitcoin(seal.txid),
        crate::rgb::compat::vout_bp_to_rgb(seal.vout),
    );
    let seal = BuilderSeal::Revealed(seal);

    let builder = ContractBuilder::with(
        Identity::default(),
        NonInflatableAsset::schema(),
        NonInflatableAsset::types(),
        NonInflatableAsset::scripts(),
        chain_net,
    );

    let contract = builder
        .add_global_state("spec", spec)?
        .add_global_state("issuedSupply", Amount::from(supply))?
        .add_global_state("terms", contract_data)?
        .add_fungible_state("assetOwner", seal, supply)?
        .issue_contract()?;

    Ok(contract.into_consignment())
}

#[allow(clippy::result_large_err)]
fn issue_uda_asset(
    params: IssueContractParams,
    _options: &IssueContractOptions,
) -> Result<Contract, IssueContractError> {
    let IssueContractParams {
        ticker,
        name,
        description,
        precision,
        seal,
        meta,
        chain_net,
        ..
    } = params.clone();

    let ticker: &'static str = Box::leak(ticker.to_string().into_boxed_str());
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());

    let precision = Precision::try_from(precision)?;
    let spec = AssetSpec::new(ticker, name, precision);
    let text = RicardianContract::from_str(&description)?;
    let terms = ContractTerms { text, media: None };

    let seal =
        Outpoint::from_str(&seal.to_string()).map_err(IssueContractError::SealsTxoutError)?;
    let seal = GenesisSeal::new_random(
        crate::rgb::compat::txid_bp_to_bitcoin(seal.txid),
        crate::rgb::compat::vout_bp_to_rgb(seal.vout),
    );
    let seal = BuilderSeal::Revealed(seal);

    let token_index = 1;
    let token_fraction = 1;
    let mut tokens_data = vec![];
    let mut allocations = vec![];

    if let Some(media_data) = meta.clone() {
        // Preview
        let preview = if let Some(media_preview) = media_data.preview {
            let ty: &'static str = Box::leak(media_preview.ty.to_string().into_boxed_str());
            let preview = base64::decode(&media_preview.source).map_err(|op| {
                IssueContractError::InvalidMedia("preview".to_string(), op.to_string())
            })?;
            let preview_len = preview.len();

            Some(EmbeddedMedia {
                ty: MediaType::with(ty),
                data: SmallBlob::try_from(preview)
                    .map_err(|_| IssueContractError::WrongPreview(preview_len))?,
            })
        } else {
            None
        };

        // Media
        let media = if let Some(media) = media_data.media {
            let ty: &'static str = Box::leak(media.ty.to_string().into_boxed_str());
            let mut digest: [u8; 32] = [0; 32];
            digest.copy_from_slice(&hex::decode(&media.source).map_err(|op| {
                IssueContractError::InvalidMedia("media".to_string(), op.to_string())
            })?);

            Some(Attachment {
                ty: MediaType::with(ty),
                digest: digest.into(),
            })
        } else {
            None
        };

        // Attachments
        let mut attachments = bmap![];
        for (index, attach) in media_data.attachments.iter().enumerate() {
            let ty: &'static str = Box::leak(attach.ty.to_string().into_boxed_str());
            let mut digest: [u8; 32] = [0; 32];
            digest.copy_from_slice(&hex::decode(&attach.source).map_err(|op| {
                IssueContractError::InvalidMedia("attachment".to_string(), op.to_string())
            })?);

            attachments.insert(
                index as u8,
                Attachment {
                    ty: MediaType::with(ty),
                    digest: digest.into(),
                },
            );
        }

        let attachments = Confined::from_checked(attachments);
        let naming = spec.clone();
        let token_data = TokenData {
            index: token_index.into(),
            name: Some(naming.name),
            ticker: Some(naming.ticker),
            preview,
            media,
            attachments,
            ..Default::default()
        };

        let allocation = Allocation::with(
            TokenIndex::from(token_index),
            OwnedFraction::from(token_fraction),
        );
        tokens_data.push(token_data);
        allocations.push(allocation);
    }

    let mut builder = ContractBuilder::with(
        Identity::default(),
        UniqueDigitalAsset::schema(),
        UniqueDigitalAsset::types(),
        UniqueDigitalAsset::scripts(),
        chain_net,
    );

    builder = builder
        .add_global_state("spec", spec)?
        .add_global_state("terms", terms)?;

    for token_data in tokens_data {
        builder = builder.add_global_state("tokens", token_data)?;
    }

    for allocation in allocations {
        builder = builder.add_data("assetOwner", seal, allocation)?;
    }

    let contract = builder.issue_contract()?;

    Ok(contract.into_consignment())
}
