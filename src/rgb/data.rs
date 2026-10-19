#![allow(clippy::unwrap_used)]
use std::collections::BTreeMap;

use crate::carbonado::error::CarbonadoError;
use crate::constants::storage_keys::{RGB_STOCK_INDEX, RGB_STOCK_STASH, RGB_STOCK_STATE};
use crate::rgb::constants::LIB_NAME_BITMASK;
use crate::rgb::constants::RGB_DATA_VERSION;
use crate::rgb::models::{ContractInfo, RgbBid, RgbOffer, RgbTransfer};
use crate::structs::SecretString;
use crate::wallet::data::{AccessCrossStore, CrossStoreError};
use bpwallet::secp256k1::{PublicKey, SecretKey};
use dbc::tapret::TapretCommitment;
use nonasync::persistence::{CloneNoPersistence, Persistence, Persisting};
use rgbcore::{ChainNet, ContractId};
use rgbstd::persistence::{MemIndex, MemStash, MemState, Stock};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use strict_encoding::{StrictDeserialize, StrictDumb, StrictSerialize};
use strict_types::{StrictDecode, StrictEncode, StrictType};

#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct RGBStockCrossStore {
    // Keys
    stash: String,
    state: String,
    index: String,

    kv: BTreeMap<String, String>,
    access: AccessCrossStore,
}

impl RGBStockCrossStore {
    pub fn with(sk: &str, network: &str) -> Result<Self, CrossStoreError> {
        let sk_bytes = hex::decode(sk)
            .map_err(|_| CrossStoreError::Fail(CarbonadoError::WrongNostrPrivateKey.to_string()))?;

        let mut buf = [0u8; 32];

        buf.copy_from_slice(&sk_bytes);
        let secret_key = SecretKey::from_byte_array(&buf)
            .map_err(|_| CrossStoreError::Fail(CarbonadoError::WrongNostrPrivateKey.to_string()))?;

        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk_bytes = public_key.serialize();
        let pk = hex::encode(pk_bytes);

        let rgb_version = String::from_utf8(RGB_DATA_VERSION.into()).unwrap();
        let stash = format!("{rgb_version}-{RGB_STOCK_STASH}");
        let state = format!("{rgb_version}-{RGB_STOCK_STATE}");
        let index = format!("{rgb_version}-{RGB_STOCK_INDEX}");

        let stash = blake3::hash(stash.as_bytes()).to_hex().to_ascii_lowercase();
        let state = blake3::hash(state.as_bytes()).to_hex().to_ascii_lowercase();
        let index = blake3::hash(index.as_bytes()).to_hex().to_ascii_lowercase();

        let access = AccessCrossStore::with(
            0,
            RGB_DATA_VERSION,
            network.to_string(),
            SecretString(sk.to_string()),
            pk,
            false,
        );

        Ok(Self {
            access,
            stash,
            state,
            index,
            ..default!()
        })
    }
}

impl CloneNoPersistence for RGBStockCrossStore {
    fn clone_no_persistence(&self) -> Self {
        let Self {
            stash,
            state,
            index,
            access,
            kv,
        } = self;

        Self {
            stash: stash.clone(),
            state: state.clone(),
            index: index.clone(),

            access: access.clone(),
            kv: kv.clone(),
        }
    }
}

pub type RGBStock = Stock<MemStash, MemState, MemIndex>;

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct RgbAccountInfo {
    pub bids: Vec<RgbBid>,
    pub offers: Vec<RgbOffer>,
    pub transfers: Vec<RgbTransfer>,
    pub contract_kits: Vec<ContractInfo>,
    pub hidden_contracts: Vec<ContractId>,
    // Persist seller pending host correlations (outpoint -> terminal)
    pub pending_hosts: BTreeMap<String, bpwallet::Terminal>,
    // Persist durable lookahead hints (keychain -> index) to avoid gap-limit blindness after restart
    pub lookahead_hints: BTreeMap<bpwallet::Keychain, bpstd::NormalIndex>,
    // Persist taproot commitments per terminal so exporters can see them across requests
    pub commits: BTreeMap<bpwallet::Terminal, TapretCommitment>,
    pub chain_net: ChainNet,
    #[serde(skip)]
    pub persistence: Option<Persistence<Self>>,
}

impl Persisting for RgbAccountInfo {
    #[inline]
    fn persistence(&self) -> Option<&Persistence<Self>> {
        self.persistence.as_ref()
    }
    #[inline]
    fn persistence_mut(&mut self) -> Option<&mut Persistence<Self>> {
        self.persistence.as_mut()
    }
    #[inline]
    fn as_mut_persistence(&mut self) -> &mut Option<Persistence<Self>> {
        &mut self.persistence
    }
}

impl CloneNoPersistence for RgbAccountInfo {
    fn clone_no_persistence(&self) -> Self {
        Self {
            bids: self.bids.clone(),
            offers: self.offers.clone(),
            transfers: self.transfers.clone(),
            contract_kits: self.contract_kits.clone(),
            hidden_contracts: self.hidden_contracts.clone(),
            pending_hosts: self.pending_hosts.clone(),
            lookahead_hints: self.lookahead_hints.clone(),
            commits: self.commits.clone(),
            chain_net: self.chain_net,
            persistence: none!(),
        }
    }
}

#[derive(Debug, StrictType, StrictDumb, StrictEncode, StrictDecode)]
#[strict_type(lib = LIB_NAME_BITMASK)]
pub struct RGBStockBlob {
    pub stash: MemStash,
    pub state: MemState,
    pub index: MemIndex,
}

impl RGBStockBlob {
    pub fn dump(stock: Stock) -> Self {
        Self {
            stash: stock.as_stash_provider().clone_no_persistence(),
            state: stock.as_state_provider().clone_no_persistence(),
            index: stock.as_index_provider().clone_no_persistence(),
        }
    }
}

impl StrictSerialize for RGBStockBlob {}
impl StrictDeserialize for RGBStockBlob {}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RgbAccountBlob {
    pub info: String,

    pub descr: String,
    pub data: String,
    pub cache: String,
    pub l2: String,
}

/// Carbonado + Postcard
#[cfg(not(target_arch = "wasm32"))]
mod server {
    use crate::rgb::data::{CrossStoreError, RGBStock, RGBStockCrossStore, RgbAccountInfo};
    use amplify::confinement::U32 as U32MAX;

    use crate::rgb::models::RgbAccount;
    use crate::wallet::data::server::handle_file;
    use crate::wallet::data::{
        AccountCrossStore, BPDescr, BPWallet, CrossStoreAsync, SyncProvider,
    };
    use amplify::confinement::Confined;
    use armor::AsciiArmor;
    use bpwallet::psbt::PsbtConstructor;
    use nonasync::persistence::{
        CloneNoPersistence, PersistenceError, PersistenceProvider, Persisting,
    };
    use postcard::{from_bytes, to_allocvec};
    use rgb::DescriptorRgb;
    use rgbstd::containers::Kit;
    use rgbstd::contract::IssuerWrapper;
    use rgbstd::persistence::{MemIndex, MemStash, MemState, Stock};
    use schemata::{CollectibleFungibleAsset, UniqueDigitalAsset};
    use std::fs;
    use strict_encoding::{StrictDeserialize, StrictSerialize};

    impl SyncProvider<AccountCrossStore> for RgbAccount {
        fn get(provider: AccountCrossStore) -> Result<Self, PersistenceError> {
            let wallet = BPWallet::load(provider.clone(), false).map_err(PersistenceError::with)?;
            let info =
                RgbAccountInfo::load(provider.clone(), false).map_err(PersistenceError::with)?;

            Ok(RgbAccount::with(wallet, info))
        }

        fn sync(&mut self, provider: &mut AccountCrossStore) -> Result<(), PersistenceError> {
            let wallet = self.wallet.clone_no_persistence();
            let data = wallet.data_l1();
            let cache = wallet.cache_l1();
            let l2 = wallet.cache_l2();

            let mut descr = wallet.descriptor().clone();
            for (terminal, tap_commit) in self.commits.clone() {
                // Apply tapret tweak into the descriptor, but keep the commit
                // available in-memory so exporters can see it immediately.
                // Re-applying on subsequent syncs should be idempotent at the descriptor level.
                descr.add_tapret_tweak(
                    terminal.into(),
                    crate::rgb::compat::tapret_commitment_rgb_from_dbc(tap_commit),
                );
            }

            let descr = BPDescr::new_standard(descr, self.wallet.network());
            let account = self.info();

            provider.store(&account).map_err(PersistenceError::with)?;
            provider.store(&descr).map_err(PersistenceError::with)?;
            provider.store(data).map_err(PersistenceError::with)?;
            provider.store(cache).map_err(PersistenceError::with)?;
            provider.store(l2).map_err(PersistenceError::with)?;

            Ok(())
        }
    }

    impl PersistenceProvider<RgbAccountInfo> for AccountCrossStore {
        fn load(&self) -> Result<RgbAccountInfo, PersistenceError> {
            self.fs_get(&self.account)
        }

        fn store(&self, object: &RgbAccountInfo) -> Result<(), PersistenceError> {
            let data = to_allocvec(object).map_err(PersistenceError::with)?;
            self.fs_set(&self.account, data)
        }
    }

    impl RGBStockCrossStore {
        fn fs_get<T>(&self, key: &str) -> Result<T, PersistenceError>
        where
            for<'de> T: serde::Serialize + serde::Deserialize<'de>,
        {
            let pk = self.access.pk();
            let sk = self.access.sk();
            let network = self.access.network();

            let sk = hex::decode(sk.to_string()).map_err(PersistenceError::with)?;

            let file_path = handle_file(pk, key, network, 0).map_err(PersistenceError::with)?;
            let descr = fs::read(&file_path).map_err(PersistenceError::with)?;

            let (_header, body) =
                carbonado::file::decode(&sk, &descr).map_err(PersistenceError::with)?;
            from_bytes::<T>(&body).map_err(PersistenceError::with)
        }

        fn fs_set(&self, key: &str, data: Vec<u8>) -> Result<(), PersistenceError> {
            let pk_hex = self.access.pk();
            let sk = self.access.sk();
            let network = self.access.network();
            let level = self.access.level();
            let version = self.access.version();

            let data = to_allocvec(&data).map_err(PersistenceError::with)?;

            let sk = hex::decode(sk.to_string()).map_err(PersistenceError::with)?;

            let (body, _encode_info) = carbonado::file::encode(&sk, &data, *level, Some(*version))
                .map_err(PersistenceError::with)?;

            let file_path =
                handle_file(pk_hex, key, network, body.len()).map_err(PersistenceError::with)?;
            fs::write(&file_path, body).map_err(PersistenceError::with)?;

            Ok(())
        }
    }

    impl SyncProvider<RGBStockCrossStore> for RGBStock {
        fn get(provider: RGBStockCrossStore) -> Result<Self, PersistenceError> {
            let provider = provider.clone_no_persistence();
            let mut stock = if let Ok(stock) =
                RGBStock::load(provider, false).map_err(PersistenceError::with)
            {
                stock
            } else {
                Stock::in_memory()
            };

            let schema = CollectibleFungibleAsset::schema();
            let lib = CollectibleFungibleAsset::scripts();
            let types = CollectibleFungibleAsset::types();

            let mut kit = Kit::default();
            kit.schemata.push(schema).map_err(PersistenceError::with)?;
            kit.scripts
                .extend(lib.into_values())
                .map_err(PersistenceError::with)?;
            kit.types = types;

            let schema = UniqueDigitalAsset::schema();
            let lib = UniqueDigitalAsset::scripts();
            let types = UniqueDigitalAsset::types();

            let rgb25 = kit.validate().unwrap();
            stock.import_kit(rgb25).map_err(PersistenceError::with)?;

            let mut kit = Kit::default();
            kit.schemata.push(schema).map_err(PersistenceError::with)?;
            kit.scripts
                .extend(lib.into_values())
                .map_err(PersistenceError::with)?;
            kit.types = types;

            let rgb21 = kit.validate().unwrap();
            stock.import_kit(rgb21).map_err(PersistenceError::with)?;

            Ok(stock)
        }

        fn sync(&mut self, provider: &mut RGBStockCrossStore) -> Result<(), PersistenceError> {
            let stash = self.as_stash_provider().clone_no_persistence();
            let state = self.as_state_provider().clone_no_persistence();
            let index = self.as_index_provider().clone_no_persistence();

            provider.store(&state)?;
            provider.store(&stash)?;
            provider.store(&index)?;
            Ok(())
        }
    }
    impl CrossStoreAsync for RGBStockCrossStore {
        async fn fetch(&mut self) -> Result<(), CrossStoreError> {
            // Ignore
            Ok(())
        }

        async fn persist(&mut self) -> Result<(), CrossStoreError> {
            // Ignore
            Ok(())
        }

        async fn destroy(&mut self) -> Result<(), CrossStoreError> {
            // Ignore
            Ok(())
        }
    }

    impl PersistenceProvider<MemStash> for RGBStockCrossStore {
        fn load(&self) -> Result<MemStash, PersistenceError> {
            let serialized: String = self.fs_get(&self.stash)?;
            let serialized =
                Vec::<u8>::from_ascii_armored_str(&serialized).map_err(PersistenceError::with)?;
            let confined = Confined::from_slice_checked(serialized.as_slice());
            let stash = MemStash::from_strict_serialized::<U32MAX>(confined)
                .map_err(PersistenceError::with)?;

            Ok(stash)
        }

        fn store(&self, object: &MemStash) -> Result<(), PersistenceError> {
            let data = MemStash::to_strict_serialized::<U32MAX>(object)
                .map_err(PersistenceError::with)?
                .to_ascii_armored_string();

            self.fs_set(&self.stash, data.as_bytes().to_vec())?;
            Ok(())
        }
    }

    impl PersistenceProvider<MemState> for RGBStockCrossStore {
        fn load(&self) -> Result<MemState, PersistenceError> {
            let serialized: String = self.fs_get(&self.state)?;
            let serialized =
                Vec::<u8>::from_ascii_armored_str(&serialized).map_err(PersistenceError::with)?;
            let confined = Confined::from_slice_checked(serialized.as_slice());
            let state = MemState::from_strict_serialized::<U32MAX>(confined)
                .map_err(PersistenceError::with)?;

            Ok(state)
        }

        fn store(&self, object: &MemState) -> Result<(), PersistenceError> {
            let data = MemState::to_strict_serialized::<U32MAX>(object)
                .map_err(PersistenceError::with)?
                .to_ascii_armored_string();

            self.fs_set(&self.state, data.as_bytes().to_vec())?;
            Ok(())
        }
    }

    impl PersistenceProvider<MemIndex> for RGBStockCrossStore {
        fn load(&self) -> Result<MemIndex, PersistenceError> {
            let serialized: String = self.fs_get(&self.index)?;
            let serialized =
                Vec::<u8>::from_ascii_armored_str(&serialized).map_err(PersistenceError::with)?;
            let confined = Confined::from_slice_checked(serialized.as_slice());
            let index = MemIndex::from_strict_serialized::<U32MAX>(confined)
                .map_err(PersistenceError::with)?;

            Ok(index)
        }

        fn store(&self, object: &MemIndex) -> Result<(), PersistenceError> {
            let data = MemIndex::to_strict_serialized::<U32MAX>(object)
                .map_err(PersistenceError::with)?
                .to_ascii_armored_string();

            self.fs_set(&self.index, data.as_bytes().to_vec())?;
            Ok(())
        }
    }
}

/// IndexedDB Storage
#[cfg(target_arch = "wasm32")]
mod client {
    use crate::carbonado::{retrieve, store};
    use crate::constants::storage_keys::{RGB_STOCK_INDEX, RGB_STOCK_STASH, RGB_STOCK_STATE};
    use crate::debug;
    use crate::rgb::constants::RGB_DEFAULT_NAME;
    use crate::rgb::data::{
        AccessCrossStore, CrossStoreError, RGBStock, RGBStockCrossStore, RgbAccountBlob,
        RgbAccountInfo,
    };
    use crate::rgb::models::RgbAccount;
    use crate::wallet::data::{
        AccountCrossStore, BPDescr, BPWallet, CrossStoreAsync, MutableProvider, SyncProvider,
    };
    use amplify::confinement::Confined;
    use amplify::confinement::U32 as U32MAX;
    use armor::AsciiArmor;
    use bpstd::Network;
    use bpwallet::psbt::PsbtConstructor;
    use nonasync::persistence::{
        CloneNoPersistence, PersistenceError, PersistenceProvider, Persisting,
    };
    use postcard::{from_bytes, to_allocvec};
    use rexie::{ObjectStore, Rexie, TransactionMode};
    use rgb::DescriptorRgb;
    use rgbcore::ChainNet;
    use rgbstd::containers::Kit;
    use rgbstd::contract::IssuerWrapper;
    use rgbstd::persistence::{MemIndex, MemStash, MemState};
    use schemata::{CollectibleFungibleAsset, UniqueDigitalAsset};
    use std::collections::BTreeMap;
    use std::io::{Error, ErrorKind};
    use strict_encoding::{StrictDeserialize, StrictSerialize};

    impl SyncProvider<AccountCrossStore> for RgbAccount {
        fn get(provider: AccountCrossStore) -> Result<Self, PersistenceError> {
            let provider = provider.clone_no_persistence();
            let wallet = BPWallet::load(provider.clone(), false).map_err(PersistenceError::with)?;

            let mut account = if let Ok(info) =
                RgbAccountInfo::load(provider.clone(), false).map_err(PersistenceError::with)
            {
                crate::debug!(format!(
                    "rgb_account.load.success pending_hosts_count={} lookahead_hints_count={} commits_count={}",
                    info.pending_hosts.len(),
                    info.lookahead_hints.len(),
                    info.commits.len()
                ));
                RgbAccount::with(wallet, info)
            } else {
                // Derive ChainNet from wallet network instead of using Default (which hardcodes Testnet4)
                let chain_net = match wallet.network() {
                    Network::Mainnet => ChainNet::BitcoinMainnet,
                    Network::Regtest => ChainNet::BitcoinRegtest,
                    Network::Signet => ChainNet::BitcoinSignet,
                    Network::Testnet3 => ChainNet::BitcoinTestnet3,
                    Network::Testnet4 => ChainNet::BitcoinTestnet4,
                };
                let info = RgbAccountInfo {
                    chain_net,
                    ..RgbAccountInfo::default()
                };
                crate::debug!(format!(
                    "rgb_account.load.fallback_to_default pending_hosts_count={} lookahead_hints_count={} commits_count={}",
                    info.pending_hosts.len(),
                    info.lookahead_hints.len(),
                    info.commits.len()
                ));
                RgbAccount::with(wallet, info)
            };

            // Clean stale pending_hosts before any operations can trigger balance()
            // This prevents cache inconsistency panics from stale metadata in previous swaps
            account.cleanup_stale_pending_hosts();

            Ok(account)
        }

        fn sync(&mut self, provider: &mut AccountCrossStore) -> Result<(), PersistenceError> {
            let wallet = self.wallet.clone_no_persistence();
            let data = wallet.data_l1();
            let cache = wallet.cache_l1();

            let mut descr = wallet.descriptor().clone();
            for (terminal, tap_commit) in self.commits.clone() {
                // Apply tapret tweak into the descriptor, but keep the commit
                // available in-memory so exporters can see it immediately.
                // Re-applying on subsequent syncs should be idempotent at the descriptor level.
                descr.add_tapret_tweak(
                    terminal.into(),
                    crate::rgb::compat::tapret_commitment_rgb_from_dbc(tap_commit),
                );
            }

            let descr = BPDescr::new_standard(descr, self.wallet.network());
            let account = self.info();

            crate::debug!(format!(
                "rgb_account.sync.saving pending_hosts_count={} lookahead_hints_count={} commits_count={}",
                account.pending_hosts.len(),
                account.lookahead_hints.len(),
                account.commits.len()
            ));

            provider.set(&account).map_err(PersistenceError::with)?;
            provider.set(&descr).map_err(PersistenceError::with)?;
            provider.set(data).map_err(PersistenceError::with)?;
            provider.set(cache).map_err(PersistenceError::with)?;
            Ok(())
        }
    }

    impl SyncProvider<RGBStockCrossStore> for RGBStock {
        fn get(provider: RGBStockCrossStore) -> Result<Self, PersistenceError> {
            let provider = provider.clone_no_persistence();
            let mut stock = RGBStock::load(provider, false).map_err(PersistenceError::with)?;

            let schema = CollectibleFungibleAsset::schema();
            let lib = CollectibleFungibleAsset::scripts();
            let types = CollectibleFungibleAsset::types();

            let mut kit = Kit::default();
            kit.schemata.push(schema).map_err(PersistenceError::with)?;
            kit.scripts
                .extend(lib.into_values())
                .map_err(PersistenceError::with)?;
            kit.types = types;

            let schema = UniqueDigitalAsset::schema();
            let lib = UniqueDigitalAsset::scripts();
            let types = UniqueDigitalAsset::types();

            let rgb25 = kit.validate().unwrap();
            stock.import_kit(rgb25).map_err(PersistenceError::with)?;

            let mut kit = Kit::default();
            kit.schemata.push(schema).map_err(PersistenceError::with)?;
            kit.scripts
                .extend(lib.into_values())
                .map_err(PersistenceError::with)?;
            kit.types = types;

            let rgb21 = kit.validate().unwrap();
            stock.import_kit(rgb21).map_err(PersistenceError::with)?;

            Ok(stock)
        }

        fn sync(&mut self, provider: &mut RGBStockCrossStore) -> Result<(), PersistenceError> {
            let stash = self.as_stash_provider();
            let state = self.as_state_provider();
            let index = self.as_index_provider();

            provider.set(stash)?;
            provider.set(state)?;
            provider.set(index)?;

            Ok(())
        }
    }
    impl CrossStoreAsync for RGBStockCrossStore {
        async fn fetch(&mut self) -> Result<(), CrossStoreError> {
            let store = self.clone_no_persistence();
            let RGBStockCrossStore {
                stash,
                state,
                index,
                ..
            } = store.clone();

            let keys = vec![stash.as_str(), state.as_str(), index.as_str()];
            let data = store.clone().get_all(keys).await?;

            for (key, data) in data {
                self.kv.insert(key, data);
            }
            Ok(())
        }

        async fn persist(&mut self) -> Result<(), CrossStoreError> {
            let store = self.clone_no_persistence();
            let RGBStockCrossStore {
                stash,
                state,
                index,
                ..
            } = store.clone();

            let stash_data = self.kv.get(&stash);
            let state_data = self.kv.get(&state);
            let index_data = self.kv.get(&index);

            let mut kv = bmap! {};
            if let Some(value) = stash_data {
                kv.insert(stash, value.clone());
            }

            if let Some(value) = state_data {
                kv.insert(state, value.clone());
            }

            if let Some(value) = index_data {
                kv.insert(index, value.clone());
            }

            self.clone().set_all(kv).await?;
            Ok(())
        }

        async fn destroy(&mut self) -> Result<(), CrossStoreError> {
            let store = self.clone_no_persistence();
            drop_db(store.access).await?;
            Ok(())
        }
    }

    impl RGBStockCrossStore {
        async fn get_all(
            self,
            keys: Vec<&str>,
        ) -> Result<BTreeMap<String, String>, CrossStoreError> {
            self.local_get(keys).await
        }

        async fn set_all(self, kv: BTreeMap<String, String>) -> Result<(), CrossStoreError> {
            self.local_set(kv).await
        }

        async fn local_get(
            self,
            keys: Vec<&str>,
        ) -> Result<BTreeMap<String, String>, CrossStoreError> {
            let rexie = create_or_open_db(self.access).await?;
            let transaction = rexie
                .transaction(&keys, TransactionMode::ReadOnly)
                .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

            let mut result = bmap! {};
            for key in keys {
                let store = transaction
                    .store(key)
                    .map_err(|err| CrossStoreError::Fail(err.to_string()));

                if store.is_err() {
                    continue;
                }
                let store = store?;
                let id = serde_wasm_bindgen::to_value(RGB_DEFAULT_NAME)
                    .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

                let retrieve = store
                    .get(id.clone())
                    .await
                    .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

                if let Some(data) = retrieve {
                    let data = serde_wasm_bindgen::from_value(data)
                        .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

                    result.insert(key.to_string(), data);
                }
            }

            transaction
                .commit()
                .await
                .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

            rexie.close();
            Ok(result)
        }

        async fn local_set(self, kv: BTreeMap<String, String>) -> Result<(), CrossStoreError> {
            let rexie = create_or_open_db(self.access).await?;

            let keys: Vec<_> = kv.keys().collect();
            let transaction = rexie
                .transaction(&keys, TransactionMode::ReadWrite)
                .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

            for (key, data) in kv {
                let obj = transaction
                    .store(&key)
                    .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

                let id = serde_wasm_bindgen::to_value(RGB_DEFAULT_NAME)
                    .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

                let data = serde_wasm_bindgen::to_value(&data)
                    .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

                obj.put(&data, Some(&id))
                    .await
                    .map_err(|err| CrossStoreError::Fail(err.to_string()))?;
            }

            transaction
                .commit()
                .await
                .map_err(|err| CrossStoreError::Fail(err.to_string()))?;

            rexie.close();
            Ok(())
        }

        #[allow(dead_code)]
        async fn server_get(
            self,
            keys: Vec<&str>,
        ) -> Result<BTreeMap<String, String>, CrossStoreError> {
            let sk = self.access.sk();
            let mut result = bmap! {};
            for key in keys {
                let (data, _) = retrieve(&sk.to_string(), key, vec![])
                    .await
                    .map_err(|err| CrossStoreError::Fail(err.to_string()))?;
                if data.is_empty() {
                    continue;
                }
                let json: String =
                    from_bytes(&data).map_err(|err| CrossStoreError::Fail(err.to_string()))?;
                result.insert(key.to_string(), json);
            }
            Ok(result)
        }

        #[allow(dead_code)]
        async fn server_set(self, kv: BTreeMap<String, String>) -> Result<(), CrossStoreError> {
            let sk = self.access.sk();
            for (key, data) in kv {
                let bin =
                    to_allocvec(&data).map_err(|err| CrossStoreError::Fail(err.to_string()))?;
                store(&sk.to_string(), &key, &bin, false, none!())
                    .await
                    .map_err(|err| CrossStoreError::Fail(err.to_string()))?;
            }
            Ok(())
        }
    }

    impl PersistenceProvider<MemStash> for RGBStockCrossStore {
        fn load(&self) -> Result<MemStash, PersistenceError> {
            if let Some(serialized) = self.kv.get(&self.stash) {
                let serialized = Vec::<u8>::from_ascii_armored_str(serialized)
                    .map_err(PersistenceError::with)?;
                let confined = Confined::from_slice_checked(serialized.as_slice());
                let stash = MemStash::from_strict_serialized::<U32MAX>(confined)
                    .map_err(PersistenceError::with)?;

                Ok(stash)
            } else {
                Ok(MemStash::in_memory())
            }
        }

        fn store(&self, _: &MemStash) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl MutableProvider<MemStash> for RGBStockCrossStore {
        fn set(&mut self, object: &MemStash) -> Result<(), PersistenceError> {
            let key = self.stash.to_string();
            let data = MemStash::to_strict_serialized::<U32MAX>(object)
                .map_err(PersistenceError::with)?
                .to_ascii_armored_string();

            self.kv.insert(key, data);
            Ok(())
        }
    }

    impl PersistenceProvider<MemState> for RGBStockCrossStore {
        fn load(&self) -> Result<MemState, PersistenceError> {
            if let Some(serialized) = self.kv.get(&self.state) {
                let serialized = Vec::<u8>::from_ascii_armored_str(serialized)
                    .map_err(PersistenceError::with)?;
                let confined = Confined::from_slice_checked(serialized.as_slice());
                let state = MemState::from_strict_serialized::<U32MAX>(confined)
                    .map_err(PersistenceError::with)?;

                Ok(state)
            } else {
                Ok(MemState::in_memory())
            }
        }

        fn store(&self, _: &MemState) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl MutableProvider<MemState> for RGBStockCrossStore {
        fn set(&mut self, object: &MemState) -> Result<(), PersistenceError> {
            let key = self.state.to_string();
            let data = MemState::to_strict_serialized::<U32MAX>(object)
                .map_err(PersistenceError::with)?
                .to_ascii_armored_string();

            self.kv.insert(key, data);
            Ok(())
        }
    }

    impl PersistenceProvider<MemIndex> for RGBStockCrossStore {
        fn load(&self) -> Result<MemIndex, PersistenceError> {
            if let Some(serialized) = self.kv.get(&self.index) {
                let serialized = Vec::<u8>::from_ascii_armored_str(serialized)
                    .map_err(PersistenceError::with)?;
                let confined = Confined::from_slice_checked(serialized.as_slice());
                let index = MemIndex::from_strict_serialized::<U32MAX>(confined)
                    .map_err(PersistenceError::with)?;

                Ok(index)
            } else {
                Ok(MemIndex::in_memory())
            }
        }

        fn store(&self, _: &MemIndex) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl MutableProvider<MemIndex> for RGBStockCrossStore {
        fn set(&mut self, object: &MemIndex) -> Result<(), PersistenceError> {
            let key = self.index.to_string();
            let data = MemIndex::to_strict_serialized::<U32MAX>(object)
                .map_err(PersistenceError::with)?
                .to_ascii_armored_string();

            self.kv.insert(key, data);
            Ok(())
        }
    }

    impl PersistenceProvider<RgbAccountInfo> for AccountCrossStore {
        fn load(&self) -> Result<RgbAccountInfo, PersistenceError> {
            if let Some(data) = self.kv.get(&self.account) {
                Ok(serde_json::from_str(data).map_err(PersistenceError::with)?)
            } else {
                Err(PersistenceError::with(Error::from(ErrorKind::InvalidData)))
            }
        }

        fn store(&self, _: &RgbAccountInfo) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl MutableProvider<RgbAccountInfo> for AccountCrossStore {
        fn set(&mut self, object: &RgbAccountInfo) -> Result<(), PersistenceError> {
            let key = self.account.to_string();
            let data = serde_json::to_string(object).map_err(PersistenceError::with)?;
            self.kv.insert(key, data);
            Ok(())
        }
    }

    impl AccountCrossStore {
        pub(crate) async fn rgb_backup(&mut self) -> Result<RgbAccountBlob, CrossStoreError> {
            let mut blob = RgbAccountBlob::default();
            let store = self.clone_no_persistence();
            let AccountCrossStore {
                descr,
                data,
                cache,
                l2,
                account,
                ..
            } = store.clone();

            if let Some(descr_data) = self.kv.get(&descr.clone()) {
                blob.descr = descr_data.clone();
            }
            if let Some(data_data) = self.kv.get(&data.clone()) {
                blob.data = data_data.clone();
            }
            if let Some(cache_data) = self.kv.get(&cache.clone()) {
                blob.cache = cache_data.clone();
            }
            if let Some(l2_data) = self.kv.get(&l2.clone()) {
                blob.l2 = l2_data.clone();
            }
            if let Some(account_data) = self.kv.get(&account.clone()) {
                blob.info = account_data.clone();
            }

            Ok(blob)
        }

        pub(crate) async fn rgb_restore(
            &mut self,
            data: RgbAccountBlob,
        ) -> Result<RgbAccount, CrossStoreError> {
            self.kv.insert(self.descr.clone(), data.descr.clone());
            self.kv.insert(self.data.clone(), data.data.clone());
            self.kv.insert(self.cache.clone(), data.cache.clone());
            self.kv.insert(self.l2.clone(), data.l2.clone());
            self.kv.insert(self.account.clone(), data.info.clone());

            let rgb_account = RgbAccount::get(self.clone()).expect("RGB wallet not found");
            Ok(rgb_account)
        }
    }

    async fn create_or_open_db(access: AccessCrossStore) -> Result<Rexie, CrossStoreError> {
        let rgb_version = String::from_utf8(access.version().to_vec()).unwrap();

        // Stock
        let stash = blake3::hash(format!("{rgb_version}-{RGB_STOCK_STASH}").as_bytes())
            .to_hex()
            .to_ascii_lowercase();

        let state = blake3::hash(format!("{rgb_version}-{RGB_STOCK_STATE}").as_bytes())
            .to_hex()
            .to_ascii_lowercase();

        let index = blake3::hash(format!("{rgb_version}-{RGB_STOCK_INDEX}").as_bytes())
            .to_hex()
            .to_ascii_lowercase();

        // Create a new database
        let network = access.network();
        let pk = access.pk();

        Rexie::builder(&format!("{network}:{pk}:stock"))
            // Set the version of the database to 1.0
            .version(1)
            // Add an object store for stash
            .add_object_store(ObjectStore::new(&stash))
            // Add an object store for state
            .add_object_store(ObjectStore::new(&state))
            // Add an object store for index
            .add_object_store(ObjectStore::new(&index))
            // Build the database
            .build()
            .await
            .map_err(|err| {
                debug!(format!("Failed to open rgb stock database: {}", err));
                CrossStoreError::Fail(err.to_string())
            })
    }

    async fn drop_db(access: AccessCrossStore) -> Result<(), CrossStoreError> {
        let network = access.network();
        let pk = access.pk();
        Rexie::delete(&format!("{network}:{pk}:stock"))
            .await
            .map_err(|err| CrossStoreError::Fail(err.to_string()))?;
        Ok(())
    }
}
