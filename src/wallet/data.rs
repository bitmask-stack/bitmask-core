use crate::bp::constants::BTC_DATA_VERSION;
use crate::carbonado::error::CarbonadoError;
use crate::constants::storage_keys::{ACCOUNT, WALLET_CACHE, WALLET_DATA, WALLET_DESCR, WALLET_L2};
use crate::rgb::constants::RGB_DATA_VERSION;
use crate::structs::SecretString;
use bpstd::XpubDerivable;
use bpstd::secp256k1::{PublicKey, SecretKey};
use bpwallet::{Layer2Empty, Wallet, WalletDescr};
use nonasync::persistence::{CloneNoPersistence, PersistenceError};
use rgb::RgbDescr;
use std::collections::BTreeMap;

pub type BPWallet = Wallet<XpubDerivable, RgbDescr<XpubDerivable>>;
pub type BPDescr = WalletDescr<XpubDerivable, RgbDescr<XpubDerivable>, Layer2Empty>;

#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct AccountCrossStore {
    // Keys
    pub descr: String,
    pub data: String,
    pub cache: String,
    pub l2: String,

    access: AccessCrossStore,
    pub account: String,
    pub kv: BTreeMap<String, String>,
}

impl AccountCrossStore {
    pub fn rgb(sk: &str, network: &str) -> Result<Self, CrossStoreError> {
        AccountCrossStore::with(sk, network, &RGB_DATA_VERSION, false)
    }

    pub fn btc(sk: &str, network: &str) -> Result<Self, CrossStoreError> {
        AccountCrossStore::with(sk, network, &BTC_DATA_VERSION, false)
    }

    pub fn rgb_recover(sk: &str, network: &str) -> Result<Self, CrossStoreError> {
        AccountCrossStore::with(sk, network, &RGB_DATA_VERSION, true)
    }

    pub fn btc_recover(sk: &str, network: &str) -> Result<Self, CrossStoreError> {
        AccountCrossStore::with(sk, network, &BTC_DATA_VERSION, true)
    }

    pub fn with(
        sk: &str,
        network: &str,
        version: &[u8; 8],
        recover: bool,
    ) -> Result<Self, CrossStoreError> {
        let sk_bytes = hex::decode(sk)
            .map_err(|_| CrossStoreError::Fail(CarbonadoError::WrongNostrPrivateKey.to_string()))?;

        let secret_key = SecretKey::from_slice(&sk_bytes)
            .map_err(|_| CrossStoreError::Fail(CarbonadoError::WrongNostrPrivateKey.to_string()))?;

        let public_key = PublicKey::from_secret_key_global(&secret_key);
        let pk_bytes = public_key.serialize();
        let pk = hex::encode(pk_bytes);

        let wallet_version = String::from_utf8(version.to_vec()).expect("Wallet version");
        let descr = format!("{wallet_version}-{WALLET_DESCR}");
        let cache = format!("{wallet_version}-{WALLET_CACHE}");
        let data = format!("{wallet_version}-{WALLET_DATA}");
        let l2 = format!("{wallet_version}-{WALLET_L2}");
        let account = format!("{wallet_version}-{ACCOUNT}");

        let descr = blake3::hash(descr.as_bytes()).to_hex().to_ascii_lowercase();
        let data = blake3::hash(data.as_bytes()).to_hex().to_ascii_lowercase();
        let cache = blake3::hash(cache.as_bytes()).to_hex().to_ascii_lowercase();
        let l2 = blake3::hash(l2.as_bytes()).to_hex().to_ascii_lowercase();
        let account = blake3::hash(account.as_bytes())
            .to_hex()
            .to_ascii_lowercase();

        let access = AccessCrossStore::with(
            0,
            *version,
            network.to_string(),
            SecretString(sk.to_string()),
            pk,
            recover,
        );

        Ok(Self {
            access,
            descr,
            cache,
            data,
            l2,
            account,
            ..default!()
        })
    }
}

impl CloneNoPersistence for AccountCrossStore {
    fn clone_no_persistence(&self) -> Self {
        let Self {
            descr,
            data,
            cache,
            l2,
            access,
            kv,
            account,
        } = self;

        Self {
            descr: descr.clone(),
            data: data.clone(),
            cache: cache.clone(),
            l2: l2.clone(),
            account: account.clone(),

            access: access.clone(),
            kv: kv.clone(),
        }
    }
}

#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum CrossStoreError {
    /// fail. {0}
    Fail(String),
}

pub trait SyncProvider<P> {
    fn get(provider: P) -> Result<Self, PersistenceError>
    where
        Self: Sized;
    fn sync(&mut self, provider: &mut P) -> Result<(), PersistenceError>;
}

#[allow(async_fn_in_trait)]
pub trait CrossStoreAsync: Sized {
    async fn fetch(&mut self) -> Result<(), CrossStoreError>;
    async fn persist(&mut self) -> Result<(), CrossStoreError>;
    async fn destroy(&mut self) -> Result<(), CrossStoreError>;
}

pub trait MutableProvider<T> {
    fn set(&mut self, object: &T) -> Result<(), PersistenceError>;
}

#[allow(async_fn_in_trait)]
pub trait BackupCrossStore<S, D> {
    async fn backup(&mut self) -> Result<D, CrossStoreError>;

    async fn restore(&mut self, data: D) -> Result<S, CrossStoreError>;
}

#[derive(Clone, Getters, Eq, PartialEq, Debug, Default)]
pub struct AccessCrossStore {
    level: u8,
    version: [u8; 8],
    network: String,
    sk: SecretString,
    pk: String,
    recover: bool,
}

impl AccessCrossStore {
    pub fn with(
        level: u8,
        version: [u8; 8],
        network: String,
        sk: SecretString,
        pk: String,
        recover: bool,
    ) -> Self {
        Self {
            level,
            version,
            network,
            sk,
            pk,
            recover,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod server {
    use crate::carbonado::error::CarbonadoError;
    use crate::info;
    use crate::wallet::data::{
        AccessCrossStore, AccountCrossStore, BPDescr, BPWallet, CrossStoreAsync, CrossStoreError,
        SyncProvider,
    };
    use bpstd::Descriptor;
    use bpstd::psbt::PsbtConstructor;
    use bpwallet::{
        Layer2Data, Layer2Descriptor, Layer2Empty, NoLayer2, WalletCache, WalletData, WalletDescr,
    };
    use nonasync::persistence::{PersistenceError, PersistenceProvider};
    use postcard::{from_bytes, to_allocvec};
    use std::fs;
    use std::io::{Error, ErrorKind};
    use std::ops::Deref;
    use std::path::PathBuf;

    impl AccountCrossStore {
        pub fn fs_get<T>(&self, key: &str) -> Result<T, PersistenceError>
        where
            for<'de> T: serde::Serialize + serde::Deserialize<'de>,
        {
            let AccessCrossStore {
                pk, sk, network, ..
            } = self.access.clone();

            let sk = hex::decode(sk.to_string()).map_err(PersistenceError::with)?;

            let file_path = handle_file(&pk, key, &network, 0).map_err(PersistenceError::with)?;
            let descr = fs::read(&file_path).map_err(PersistenceError::with)?;

            let (_header, body) =
                carbonado::file::decode(&sk, &descr).map_err(PersistenceError::with)?;
            from_bytes::<T>(&body).map_err(PersistenceError::with)
        }

        pub fn fs_set(&self, key: &str, data: Vec<u8>) -> Result<(), PersistenceError> {
            let AccessCrossStore {
                pk: pk_hex,
                sk,
                network,
                level,
                version,
                ..
            } = self.access.clone();

            let sk = hex::decode(sk.to_string()).map_err(PersistenceError::with)?;

            let (body, _encode_info) = carbonado::file::encode(&sk, &data, level, Some(version))
                .map_err(PersistenceError::with)?;

            let file_path =
                handle_file(&pk_hex, key, &network, body.len()).map_err(PersistenceError::with)?;
            fs::write(&file_path, body).map_err(PersistenceError::with)?;

            Ok(())
        }
    }

    impl SyncProvider<AccountCrossStore> for BPWallet {
        fn get(provider: AccountCrossStore) -> Result<Self, PersistenceError> {
            let wallet = BPWallet::load(provider, false).map_err(PersistenceError::with)?;
            Ok(wallet)
        }

        fn sync(&mut self, provider: &mut AccountCrossStore) -> Result<(), PersistenceError> {
            let descr = self.deref().descriptor();
            let descr = BPDescr::new_standard(descr.clone(), self.network());

            provider.store(&descr).map_err(PersistenceError::with)?;
            provider
                .store(self.data_l1())
                .map_err(PersistenceError::with)?;
            provider
                .store(self.cache_l1())
                .map_err(PersistenceError::with)?;
            provider
                .store(self.cache_l2())
                .map_err(PersistenceError::with)?;
            Ok(())
        }
    }

    impl CrossStoreAsync for AccountCrossStore {
        async fn fetch(&mut self) -> Result<(), CrossStoreError> {
            // Ignore
            Ok(())
        }

        async fn persist(&mut self) -> Result<(), CrossStoreError> {
            Ok(())
        }

        async fn destroy(&mut self) -> Result<(), CrossStoreError> {
            // Ignore
            Ok(())
        }
    }

    impl<K, D: Descriptor<K>, L2: Layer2Descriptor> PersistenceProvider<WalletDescr<K, D, L2>>
        for AccountCrossStore
    where
        for<'de> WalletDescr<K, D, L2>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> D: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn load(&self) -> Result<WalletDescr<K, D, L2>, PersistenceError> {
            let data: String = self.fs_get(&self.descr)?;
            serde_json::from_str::<WalletDescr<K, D, L2>>(&data).map_err(PersistenceError::with)
        }

        fn store(&self, object: &WalletDescr<K, D, L2>) -> Result<(), PersistenceError> {
            let data = serde_json::to_string(&object).map_err(PersistenceError::with)?;
            let data = to_allocvec(&data).map_err(PersistenceError::with)?;
            self.fs_set(&self.descr, data)
        }
    }

    impl PersistenceProvider<WalletCache<Layer2Empty>> for AccountCrossStore
    where
        for<'de> WalletCache<Layer2Empty>: serde::Serialize + serde::Deserialize<'de>,
    {
        fn load(&self) -> Result<WalletCache<Layer2Empty>, PersistenceError> {
            self.fs_get(&self.cache)
        }

        fn store(&self, object: &WalletCache<Layer2Empty>) -> Result<(), PersistenceError> {
            let data = to_allocvec(object).map_err(PersistenceError::with)?;
            self.fs_set(&self.cache, data)
        }
    }

    impl<L2: Layer2Data> PersistenceProvider<WalletData<L2>> for AccountCrossStore
    where
        for<'de> WalletData<L2>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn load(&self) -> Result<WalletData<L2>, PersistenceError> {
            self.fs_get(&self.data)
        }

        fn store(&self, object: &WalletData<L2>) -> Result<(), PersistenceError> {
            let data = to_allocvec(object).map_err(PersistenceError::with)?;
            self.fs_set(&self.data, data)
        }
    }

    impl PersistenceProvider<NoLayer2> for AccountCrossStore
    where
        for<'de> NoLayer2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn load(&self) -> Result<NoLayer2, PersistenceError> {
            // self.fs_get(&self.l2)
            // Nothing to do
            Ok(none!())
        }

        fn store(&self, _: &NoLayer2) -> Result<(), PersistenceError> {
            // let data = to_allocvec(object).map_err(PersistenceError::with)?;
            // self.fs_set(&self.l2, data)
            Ok(())
        }
    }

    impl PersistenceProvider<Layer2Empty> for AccountCrossStore {
        fn load(&self) -> Result<Layer2Empty, PersistenceError> {
            // Nothing to do
            Ok(none!())
        }

        fn store(&self, _: &Layer2Empty) -> Result<(), PersistenceError> {
            // Nothing to do
            Ok(())
        }
    }

    pub fn handle_file(
        pk: &str,
        name: &str,
        network: &str,
        bytes: usize,
    ) -> Result<PathBuf, CarbonadoError> {
        let mut final_name = name.to_string();
        let networks = ["bitcoin", "testnet", "signet", "regtest"];
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

        fs::create_dir_all(filedir).map_err(|_| {
            CarbonadoError::StdIoError(Error::new(
                ErrorKind::NotFound,
                format!("Cannot create filepath to carbonado file {name}"),
            ))
        })?;

        let message = if bytes == 0 {
            format!("read {}", filepath.to_string_lossy())
        } else {
            format!("write {bytes} bytes to {}", filepath.to_string_lossy())
        };
        info!(message);
        Ok(filepath)
    }
}

/// IndexedDB Storage
#[cfg(target_arch = "wasm32")]
mod client {
    use crate::carbonado::{retrieve, store};
    use crate::constants::storage_keys::{
        ACCOUNT, WALLET_CACHE, WALLET_DATA, WALLET_DESCR, WALLET_L2,
    };
    use crate::debug;
    use crate::rgb::constants::{RGB_DATA_VERSION, RGB_DEFAULT_NAME};
    use crate::wallet::data::{
        AccessCrossStore, AccountCrossStore, BPDescr, BPWallet, CrossStoreAsync, CrossStoreError,
        MutableProvider, SyncProvider,
    };
    use bpstd::Descriptor;
    use bpwallet::psbt::PsbtConstructor;
    use bpwallet::{
        Layer2Cache, Layer2Data, Layer2Descriptor, Layer2Empty, NoLayer2, WalletCache, WalletData,
        WalletDescr,
    };
    use nonasync::persistence::{CloneNoPersistence, PersistenceError, PersistenceProvider};
    use postcard::{from_bytes, to_allocvec};
    use rexie::{ObjectStore, Rexie, TransactionMode};
    use std::collections::BTreeMap;
    use std::io::{Error, ErrorKind};
    use std::ops::Deref;

    impl SyncProvider<AccountCrossStore> for BPWallet {
        fn get(provider: AccountCrossStore) -> Result<Self, PersistenceError> {
            let provider = provider.clone_no_persistence();
            let wallet = BPWallet::load(provider, false).map_err(PersistenceError::with)?;
            Ok(wallet)
        }

        fn sync(&mut self, provider: &mut AccountCrossStore) -> Result<(), PersistenceError> {
            let data = self.data_l1();
            let cache = self.cache_l1();

            let descr = self.deref().descriptor();
            let descr = BPDescr::new_standard(descr.clone(), self.network());

            provider.set(&descr).map_err(PersistenceError::with)?;
            provider.set(data).map_err(PersistenceError::with)?;
            provider.set(cache).map_err(PersistenceError::with)?;
            Ok(())
        }
    }

    impl CrossStoreAsync for AccountCrossStore {
        async fn fetch(&mut self) -> Result<(), CrossStoreError> {
            let store = self.clone_no_persistence();
            let AccountCrossStore {
                descr,
                data,
                cache,
                account,
                ..
            } = store.clone();

            let keys = vec![
                descr.as_str(),
                data.as_str(),
                cache.as_str(),
                account.as_str(),
            ];
            let data = store.clone().get_all(keys).await?;

            for (key, data) in data {
                self.kv.insert(key, data);
            }
            Ok(())
        }

        async fn persist(&mut self) -> Result<(), CrossStoreError> {
            let store = self.clone_no_persistence();
            let AccountCrossStore {
                descr,
                data,
                cache,
                l2,
                account,
                ..
            } = store.clone();

            let descr_data = self.kv.get(&descr.clone());
            let data_data = self.kv.get(&data);
            let cache_data = self.kv.get(&cache);
            let l2_data = self.kv.get(&l2);
            let account_data = self.kv.get(&account);

            let mut kv = bmap! {};
            if let Some(value) = descr_data {
                kv.insert(descr, value.clone());
            }

            if let Some(value) = data_data {
                kv.insert(data, value.clone());
            }

            if let Some(value) = cache_data {
                kv.insert(cache, value.clone());
            }

            if let Some(value) = l2_data {
                kv.insert(l2, value.clone());
            }
            if let Some(value) = account_data {
                kv.insert(account, value.clone());
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

    impl AccountCrossStore {
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

    impl<K, D: Descriptor<K>, L2: Layer2Descriptor> PersistenceProvider<WalletDescr<K, D, L2>>
        for AccountCrossStore
    where
        for<'de> WalletDescr<K, D, L2>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> D: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn load(&self) -> Result<WalletDescr<K, D, L2>, PersistenceError> {
            if let Some(data) = self.kv.get(&self.descr) {
                Ok(serde_json::from_str(data).map_err(PersistenceError::with)?)
            } else {
                Err(PersistenceError::with(Error::from(ErrorKind::InvalidData)))
            }
        }

        fn store(&self, _: &WalletDescr<K, D, L2>) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl<K, D: Descriptor<K>, L2: Layer2Descriptor> MutableProvider<WalletDescr<K, D, L2>>
        for AccountCrossStore
    where
        for<'de> WalletDescr<K, D, L2>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> D: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn set(&mut self, object: &WalletDescr<K, D, L2>) -> Result<(), PersistenceError> {
            let key = self.descr.to_string();
            let data = serde_json::to_string(object).map_err(PersistenceError::with)?;
            self.kv.insert(key, data);
            Ok(())
        }
    }

    impl<L2: Layer2Data> PersistenceProvider<WalletData<L2>> for AccountCrossStore
    where
        for<'de> WalletData<L2>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn load(&self) -> Result<WalletData<L2>, PersistenceError> {
            if let Some(data) = self.kv.get(&self.data) {
                Ok(serde_json::from_str(data).map_err(PersistenceError::with)?)
            } else {
                Err(PersistenceError::with(Error::from(ErrorKind::InvalidData)))
            }
        }

        fn store(&self, _: &WalletData<L2>) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl<L2: Layer2Data> MutableProvider<WalletData<L2>> for AccountCrossStore
    where
        for<'de> WalletData<L2>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn set(&mut self, object: &WalletData<L2>) -> Result<(), PersistenceError> {
            let key = self.data.to_string();
            let data = serde_json::to_string(object).map_err(PersistenceError::with)?;

            self.kv.insert(key, data);
            Ok(())
        }
    }

    impl<L2: Layer2Cache> PersistenceProvider<WalletCache<L2>> for AccountCrossStore
    where
        for<'de> WalletCache<L2>: serde::Serialize + serde::Deserialize<'de>,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn load(&self) -> Result<WalletCache<L2>, PersistenceError> {
            if let Some(data) = self.kv.get(&self.cache) {
                Ok(serde_json::from_str(data).map_err(PersistenceError::with)?)
            } else {
                Err(PersistenceError::with(Error::from(ErrorKind::InvalidData)))
            }
        }

        fn store(&self, _: &WalletCache<L2>) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl<L2: Layer2Cache> MutableProvider<WalletCache<L2>> for AccountCrossStore
    where
        for<'de> WalletCache<L2>: serde::Serialize,
        for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn set(&mut self, object: &WalletCache<L2>) -> Result<(), PersistenceError> {
            let key = self.cache.to_string();
            let data = serde_json::to_string(object).map_err(PersistenceError::with)?;

            self.kv.insert(key, data);
            Ok(())
        }
    }

    impl PersistenceProvider<NoLayer2> for AccountCrossStore
    where
        for<'de> NoLayer2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn load(&self) -> Result<NoLayer2, PersistenceError> {
            Ok(none!())
        }

        fn store(&self, _: &NoLayer2) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl MutableProvider<NoLayer2> for AccountCrossStore
    where
        for<'de> NoLayer2: serde::Serialize + serde::Deserialize<'de>,
    {
        fn set(&mut self, _: &NoLayer2) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl PersistenceProvider<Layer2Empty> for AccountCrossStore {
        fn load(&self) -> Result<Layer2Empty, PersistenceError> {
            // Nothing to do
            Ok(none!())
        }

        fn store(&self, _: &Layer2Empty) -> Result<(), PersistenceError> {
            // Nothing to do
            Ok(())
        }
    }

    impl MutableProvider<Layer2Empty> for AccountCrossStore {
        fn set(&mut self, _: &Layer2Empty) -> Result<(), PersistenceError> {
            // Nothing to do
            Ok(())
        }
    }

    async fn create_or_open_db(access: AccessCrossStore) -> Result<Rexie, CrossStoreError> {
        let wallet_version = String::from_utf8(access.version().to_vec()).unwrap();

        let ty = if access.version() == &RGB_DATA_VERSION {
            "rgb"
        } else {
            "btc"
        };

        // Wallet
        let descr = blake3::hash(format!("{wallet_version}-{WALLET_DESCR}").as_bytes())
            .to_hex()
            .to_ascii_lowercase();
        let cache = blake3::hash(format!("{wallet_version}-{WALLET_CACHE}").as_bytes())
            .to_hex()
            .to_ascii_lowercase();
        let data = blake3::hash(format!("{wallet_version}-{WALLET_DATA}").as_bytes())
            .to_hex()
            .to_ascii_lowercase();
        let l2 = blake3::hash(format!("{wallet_version}-{WALLET_L2}").as_bytes())
            .to_hex()
            .to_ascii_lowercase();

        // Account
        let account = blake3::hash(format!("{wallet_version}-{ACCOUNT}").as_bytes())
            .to_hex()
            .to_ascii_lowercase();

        let recover_tag = if access.recover { "recover:" } else { "" };

        // Create a new database
        let AccessCrossStore { pk, network, .. } = access;
        Rexie::builder(&format!("{recover_tag}{network}:{pk}:{ty}"))
            // Set the version of the database to 1.0
            .version(1)
            // Add an object store for descr
            .add_object_store(ObjectStore::new(&descr))
            // Add an object store for data
            .add_object_store(ObjectStore::new(&data))
            // Add an object store for cache
            .add_object_store(ObjectStore::new(&cache))
            // Add an object store for layer2
            .add_object_store(ObjectStore::new(&l2))
            // Add an object store for account
            .add_object_store(ObjectStore::new(&account))
            // Build the database
            .build()
            .await
            .map_err(|err| {
                debug!(format!("Failed to open rgb stock database: {}", err));
                CrossStoreError::Fail(err.to_string())
            })
    }

    async fn drop_db(access: AccessCrossStore) -> Result<(), CrossStoreError> {
        let AccessCrossStore {
            pk,
            network,
            version,
            ..
        } = access;
        let ty = if version == RGB_DATA_VERSION {
            "rgb"
        } else {
            "btc"
        };

        Rexie::delete(&format!("{network}:{pk}:{ty}"))
            .await
            .map_err(|err| CrossStoreError::Fail(err.to_string()))?;
        Ok(())
    }
}
