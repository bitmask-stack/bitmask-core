use nonasync::persistence::{CloneNoPersistence, Persistence, Persisting};
use rgbcore::ChainNet;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct BtcAccountInfo {
    pub chain_net: ChainNet,

    #[serde(skip)]
    pub persistence: Option<Persistence<Self>>,
}

impl CloneNoPersistence for BtcAccountInfo {
    fn clone_no_persistence(&self) -> Self {
        Self {
            chain_net: self.chain_net,
            persistence: none!(),
        }
    }
}

impl Persisting for BtcAccountInfo {
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BtcAccountBlob {
    pub info: String,

    pub descr: String,
    pub data: String,
    pub cache: String,
    pub l2: String,
}

/// Carbonado + Postcard
#[cfg(not(target_arch = "wasm32"))]
mod server {
    use crate::bp::data::BtcAccountInfo;
    use crate::bp::models::BtcAccount;
    use crate::wallet::data::{AccountCrossStore, BPDescr, BPWallet, SyncProvider};
    use bpstd::psbt::PsbtConstructor;
    use nonasync::persistence::{
        CloneNoPersistence, PersistenceError, PersistenceProvider, Persisting,
    };
    use postcard::to_allocvec;

    impl SyncProvider<AccountCrossStore> for BtcAccount {
        fn get(provider: AccountCrossStore) -> Result<Self, PersistenceError> {
            let wallet = BPWallet::load(provider.clone(), false).map_err(PersistenceError::with)?;
            let info =
                BtcAccountInfo::load(provider.clone(), false).map_err(PersistenceError::with)?;

            Ok(BtcAccount::with(wallet, info))
        }

        fn sync(&mut self, provider: &mut AccountCrossStore) -> Result<(), PersistenceError> {
            let wallet = self.wallet.clone_no_persistence();
            let data = wallet.data_l1();
            let cache = wallet.cache_l1();
            let l2 = wallet.cache_l2();

            let descr = wallet.descriptor().clone();

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

    impl PersistenceProvider<BtcAccountInfo> for AccountCrossStore {
        fn load(&self) -> Result<BtcAccountInfo, PersistenceError> {
            self.fs_get(&self.account)
        }

        fn store(&self, object: &BtcAccountInfo) -> Result<(), PersistenceError> {
            let data = to_allocvec(object).map_err(PersistenceError::with)?;
            self.fs_set(&self.account, data)
        }
    }
}

/// IndexedDB Storage
#[cfg(target_arch = "wasm32")]
mod client {
    use crate::bp::data::{BtcAccountBlob, BtcAccountInfo};
    use crate::bp::models::BtcAccount;
    use crate::wallet::data::{
        AccountCrossStore, BPDescr, BPWallet, CrossStoreError, MutableProvider, SyncProvider,
    };
    use bpstd::psbt::PsbtConstructor;
    use nonasync::persistence::{
        CloneNoPersistence, PersistenceError, PersistenceProvider, Persisting,
    };
    use std::io::{Error, ErrorKind};

    impl SyncProvider<AccountCrossStore> for BtcAccount {
        fn get(provider: AccountCrossStore) -> Result<Self, PersistenceError> {
            let provider = provider.clone_no_persistence();
            let wallet = BPWallet::load(provider.clone(), false).map_err(PersistenceError::with)?;

            let account = if let Ok(info) =
                BtcAccountInfo::load(provider, false).map_err(PersistenceError::with)
            {
                BtcAccount::with(wallet, info)
            } else {
                let info = BtcAccountInfo::default();
                BtcAccount::with(wallet, info)
            };
            Ok(account)
        }

        fn sync(&mut self, provider: &mut AccountCrossStore) -> Result<(), PersistenceError> {
            let wallet = self.wallet.clone_no_persistence();
            let data = wallet.data_l1();
            let cache = wallet.cache_l1();

            let descr = wallet.descriptor().clone();
            let descr = BPDescr::new_standard(descr, self.wallet.network());
            let account = self.info();

            provider.set(&account).map_err(PersistenceError::with)?;
            provider.set(&descr).map_err(PersistenceError::with)?;
            provider.set(data).map_err(PersistenceError::with)?;
            provider.set(cache).map_err(PersistenceError::with)?;
            Ok(())
        }
    }

    impl PersistenceProvider<BtcAccountInfo> for AccountCrossStore {
        fn load(&self) -> Result<BtcAccountInfo, PersistenceError> {
            if let Some(data) = self.kv.get(&self.account) {
                Ok(serde_json::from_str(data).map_err(PersistenceError::with)?)
            } else {
                Err(PersistenceError::with(Error::from(ErrorKind::InvalidData)))
            }
        }

        fn store(&self, _: &BtcAccountInfo) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    impl MutableProvider<BtcAccountInfo> for AccountCrossStore {
        fn set(&mut self, object: &BtcAccountInfo) -> Result<(), PersistenceError> {
            let key = self.account.to_string();
            let data = serde_json::to_string(object).map_err(PersistenceError::with)?;
            self.kv.insert(key, data);
            Ok(())
        }
    }

    impl AccountCrossStore {
        pub(crate) async fn btc_backup(&mut self) -> Result<BtcAccountBlob, CrossStoreError> {
            let mut blob = BtcAccountBlob::default();
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

        pub(crate) async fn btc_restore(
            &mut self,
            data: BtcAccountBlob,
        ) -> Result<BtcAccount, CrossStoreError> {
            self.kv.insert(self.descr.clone(), data.descr.clone());
            self.kv.insert(self.data.clone(), data.data.clone());
            self.kv.insert(self.cache.clone(), data.cache.clone());
            self.kv.insert(self.l2.clone(), data.l2.clone());
            self.kv.insert(self.account.clone(), data.info.clone());

            let btc_account = BtcAccount::get(self.clone()).expect("RGB wallet not found");
            Ok(btc_account)
        }
    }
}
