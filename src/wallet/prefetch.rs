use crate::bp::models::BtcAccount;
use crate::constants::BITCOIN_EXPLORER_API;
use crate::rgb::models::RgbAccount;
use crate::wallet::data::AccountCrossStore;
#[cfg(target_arch = "wasm32")]
use crate::wallet::data::MutableProvider;
use bpstd::psbt::PsbtConstructor;
use bpstd::{
    Address, AddressNetwork, DeriveScripts, DerivedAddr, IdxBase, Keychain, LockTime, Network,
    NormalIndex, Outpoint, SeqNo, TxVer, Witness,
};
use bpwallet::{
    Layer2Empty, MiningInfo, Party, TxCredit, TxDebit, TxStatus, WalletAddr, WalletCache, WalletTx,
};
use esplora::{AsyncClient, Error};
#[cfg(not(target_arch = "wasm32"))]
use nonasync::persistence::PersistenceProvider;
use nonasync::persistence::{CloneNoPersistence, PersistenceError, Persisting};
use rgbcore::ChainNet;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::future::Future;
use std::num::NonZeroU32;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, From, Error, Display)]
#[display(doc_comments)]
pub enum PrefetchError {
    /// esplora error. {0}
    EsploraError(#[from] Error),
    /// invalid network.
    InvalidNetwork,
    /// broken logic error.
    BrokenLogicError,
    /// persistent logic error. {0}
    Persistent(PersistenceError),
    /// try from int error. {0}
    TryFromIntError(#[from] std::num::TryFromIntError),
}

pub trait PrefetchSync {
    fn fetch(
        self,
        provider: &mut AccountCrossStore,
        force: bool,
    ) -> impl Future<Output = Result<(), PrefetchError>>;
}

impl PrefetchSync for RgbAccount {
    async fn fetch(
        self,
        provider: &mut AccountCrossStore,
        recover: bool,
    ) -> Result<(), PrefetchError> {
        let network = match self.chain_net() {
            ChainNet::BitcoinMainnet => Network::Mainnet,
            ChainNet::BitcoinRegtest => Network::Regtest,
            ChainNet::BitcoinSignet => Network::Signet,
            ChainNet::BitcoinTestnet3 => Network::Testnet3,
            ChainNet::BitcoinTestnet4 => Network::Testnet4,
            _ => {
                return Err(PrefetchError::InvalidNetwork);
            }
        };

        let network = AddressNetwork::from(network);
        let keychains = [Keychain::with(9), Keychain::with(10)];

        // 1 . Verify address
        let mut script_pubkeys = vec![];
        let cache = self.wallet.cache_l1().clone_no_persistence();

        // TODO: 2. Verify contracts
        for keychain in keychains {
            let last_index = if recover {
                NormalIndex::normal(10)
            } else {
                self.wallet.last_derivation_index(keychain)
            };
            let last_cached_index = cache
                .utxos()
                .filter(|x| !x.status.is_mined() && x.terminal.keychain == keychain)
                .map(|x| x.terminal.index)
                .min();

            let last_cached_index = if let Some(index) = last_cached_index {
                index
            } else {
                NormalIndex::normal(0)
            };

            match last_cached_index.cmp(&last_index) {
                Ordering::Less | Ordering::Equal => {
                    for index in last_cached_index.index()..=last_index.index() {
                        let address = self
                            .wallet
                            .derive_address(network, keychain, NormalIndex::normal(index as u16))
                            .map(|x| {
                                DerivedAddr::new(x, keychain, NormalIndex::normal(index as u16))
                            });
                        script_pubkeys.extend_from_slice(&address.into_iter().collect::<Vec<_>>());
                    }
                }
                _ => return Err(PrefetchError::BrokenLogicError),
            }
        }

        // 3 . Verify debits
        let mut cache = self.wallet.cache_l1().clone_no_persistence();
        cache
            .pull(self.wallet.network(), script_pubkeys, recover)
            .await
            .expect("esplora");

        cache.store().map_err(PrefetchError::Persistent)?;

        #[cfg(not(target_arch = "wasm32"))]
        provider.store(&cache).map_err(PrefetchError::Persistent)?;

        #[cfg(target_arch = "wasm32")]
        provider.set(&cache).map_err(PrefetchError::Persistent)?;

        Ok(())
    }
}

impl PrefetchSync for BtcAccount {
    async fn fetch(
        self,
        provider: &mut AccountCrossStore,
        recover: bool,
    ) -> Result<(), PrefetchError> {
        let network = match self.chain_net() {
            ChainNet::BitcoinMainnet => Network::Mainnet,
            ChainNet::BitcoinRegtest => Network::Regtest,
            ChainNet::BitcoinSignet => Network::Signet,
            ChainNet::BitcoinTestnet3 => Network::Testnet3,
            ChainNet::BitcoinTestnet4 => Network::Testnet4,
            _ => {
                return Err(PrefetchError::BrokenLogicError);
            }
        };

        let network = AddressNetwork::from(network);
        let keychains = if recover {
            let indexes = [0, 1, 10, 20, 30];
            indexes.map(Keychain::with).to_vec()
        } else {
            [Keychain::with(0), Keychain::with(1)].to_vec()
        };

        // 1 . Verify address
        let mut script_pubkeys: Vec<DerivedAddr> = Vec::new();
        let mut cache = self.wallet.cache_l1().clone_no_persistence();

        for keychain in keychains {
            let last_index = if recover {
                NormalIndex::normal(5)
            } else {
                self.wallet.last_derivation_index(keychain)
            };
            let last_cached_index = cache
                .utxos()
                .filter(|x| !x.status.is_mined() && x.terminal.keychain == keychain)
                .map(|x| x.terminal.index)
                .min();

            let last_cached_index = if let Some(index) = last_cached_index {
                index
            } else {
                NormalIndex::normal(0)
            };

            match last_cached_index.cmp(&last_index) {
                Ordering::Less | Ordering::Equal => {
                    for index in last_cached_index.index()..=last_index.index() {
                        let addresses = self
                            .wallet
                            .derive_address(network, keychain, NormalIndex::normal(index as u16))
                            .map(|x| {
                                DerivedAddr::new(x, keychain, NormalIndex::normal(index as u16))
                            });
                        script_pubkeys.extend(addresses);
                    }
                }
                _ => return Err(PrefetchError::BrokenLogicError),
            }
        }

        // 1 . Verify debits
        cache
            .pull(self.wallet.network(), script_pubkeys, recover)
            .await
            .expect("esplora");

        cache.store().map_err(|_| PrefetchError::BrokenLogicError)?;

        #[cfg(not(target_arch = "wasm32"))]
        provider
            .store(&cache)
            .map_err(|_| PrefetchError::BrokenLogicError)?;

        #[cfg(target_arch = "wasm32")]
        provider
            .set(&cache)
            .map_err(|_| PrefetchError::BrokenLogicError)?;

        Ok(())
    }
}

pub trait FetchData {
    fn pull(
        &mut self,
        network: Network,
        address: Vec<DerivedAddr>,
        recover: bool,
    ) -> impl Future<Output = Result<(), PrefetchError>>;
}

impl FetchData for WalletCache<Layer2Empty> {
    async fn pull(
        &mut self,
        network: Network,
        address: Vec<DerivedAddr>,
        recover: bool,
    ) -> Result<(), PrefetchError> {
        let explorer_url = BITCOIN_EXPLORER_API.read().await.to_string();
        let client = AsyncClient::from_builder(esplora::Builder::new(&explorer_url))
            .map_err(PrefetchError::EsploraError)?;

        let mut errors = vec![];
        let mut address_index = BTreeMap::new();

        let mut empty_count = 0usize;
        for derive in address {
            let script = derive.addr.script_pubkey();
            let mut txids = Vec::new();
            match get_scripthash_txs_all(&client, &derive).await {
                Err(err) => {
                    errors.push(err);
                    break;
                }
                Ok(txes) if txes.is_empty() => {
                    if recover {
                        continue;
                    } else if empty_count >= 5 {
                        break;
                    } else {
                        empty_count += 1;
                    }
                }
                Ok(txes) => {
                    empty_count = 0;
                    txids = txes.iter().map(|tx| tx.txid).collect();
                    let wallet_utxos = txes
                        .into_iter()
                        .map(|tx| WalletTx {
                            txid: tx.txid,
                            status: {
                                if let esplora::TxStatus {
                                    confirmed: true,
                                    block_height: Some(height),
                                    block_hash: Some(hash),
                                    block_time: Some(ts),
                                } = tx.status
                                {
                                    TxStatus::Mined(MiningInfo {
                                        height: NonZeroU32::try_from(height)
                                            .unwrap_or(NonZeroU32::MIN),
                                        time: ts,
                                        block_hash: hash,
                                    })
                                } else {
                                    TxStatus::Mempool
                                }
                            },
                            inputs: tx
                                .vin
                                .into_iter()
                                .map(|vin| TxCredit {
                                    outpoint: Outpoint::new(vin.txid, vin.vout),
                                    sequence: SeqNo::from_consensus_u32(vin.sequence),
                                    coinbase: vin.is_coinbase,
                                    script_sig: vin.scriptsig,
                                    witness: Witness::from_consensus_stack(vin.witness),
                                    value: vin
                                        .prevout
                                        .as_ref()
                                        .map(|prevout| prevout.value)
                                        .unwrap_or_default()
                                        .into(),
                                    payer: vin
                                        .prevout
                                        .map(|prevout| Party::Unknown(prevout.scriptpubkey))
                                        .unwrap_or(Party::Subsidy),
                                })
                                .collect(),
                            outputs: tx
                                .vout
                                .into_iter()
                                .enumerate()
                                .map(|(n, vout)| TxDebit {
                                    outpoint: Outpoint::new(tx.txid, n as u32),
                                    beneficiary: Party::from(vout.scriptpubkey),
                                    value: vout.value.into(),
                                    spent: None,
                                })
                                .collect(),
                            fee: tx.fee.into(),
                            size: tx.size,
                            weight: tx.weight,
                            version: TxVer::from_consensus_i32(tx.version),
                            locktime: LockTime::from_consensus_u32(tx.locktime),
                        })
                        .map(|tx| (tx.txid, tx));
                    for wallet_utxo in wallet_utxos {
                        if self.tx.contains_key(&wallet_utxo.0) {
                            self.tx.remove(&wallet_utxo.0);
                        }
                        self.tx.insert(wallet_utxo.0, wallet_utxo.1);
                    }
                }
            }

            let wallet_addr = WalletAddr::<i64>::from(derive);
            address_index.insert(script, (wallet_addr, txids));
        }

        // TODO: Update headers & tip
        for (script, (wallet_addr, txids)) in &mut address_index {
            for txid in txids {
                let mut tx = self.tx.remove(txid).expect("broken logic");
                for debit in &mut tx.outputs {
                    let Some(s) = debit.beneficiary.script_pubkey() else {
                        continue;
                    };
                    if &s == script {
                        self.utxo.insert(debit.outpoint);
                        debit.beneficiary = Party::from_wallet_addr(wallet_addr);
                        wallet_addr.used = wallet_addr.used.saturating_add(1);
                        wallet_addr.volume.saturating_add_assign(debit.value);
                        wallet_addr.balance = wallet_addr
                            .balance
                            .saturating_add(debit.value.sats().try_into().expect("sats overflow"));
                    } else if debit.beneficiary.is_unknown() {
                        Address::with(&s, network)
                            .map(|addr| {
                                debit.beneficiary = Party::Counterparty(addr);
                            })
                            .ok();
                    }
                }
                self.tx.insert(tx.txid, tx);
            }
        }

        for (script, (wallet_addr, txids)) in &mut address_index {
            for txid in txids {
                let mut tx = self.tx.remove(txid).expect("broken logic");
                for credit in &mut tx.inputs {
                    let Some(s) = credit.payer.script_pubkey() else {
                        continue;
                    };
                    if &s == script {
                        credit.payer = Party::from_wallet_addr(wallet_addr);
                        wallet_addr.balance = wallet_addr
                            .balance
                            .saturating_sub(credit.value.sats().try_into().expect("sats overflow"));
                    } else if credit.payer.is_unknown() {
                        Address::with(&s, network)
                            .map(|addr| {
                                credit.payer = Party::Counterparty(addr);
                            })
                            .ok();
                    }
                    if let Some(prev_tx) = self.tx.get_mut(&credit.outpoint.txid)
                        && let Some(txout) =
                            prev_tx.outputs.get_mut(credit.outpoint.vout_u32() as usize)
                    {
                        let outpoint = txout.outpoint;
                        if tx.status.is_mined() || matches!(tx.status, TxStatus::Mempool) {
                            self.utxo.remove(&outpoint);
                        }
                        txout.spent = Some(credit.outpoint.into())
                    }
                }
                self.tx.insert(tx.txid, tx);
            }
            self.addr
                .entry(wallet_addr.terminal.keychain)
                .or_default()
                .insert(wallet_addr.expect_transmute());
        }

        Ok(())
    }
}

#[allow(clippy::result_large_err)]
async fn get_scripthash_txs_all(
    client: &AsyncClient,
    derive: &DerivedAddr,
) -> Result<Vec<esplora::Tx>, Error> {
    const PAGE_SIZE: usize = 25;
    let mut res = Vec::new();
    let mut last_seen = None;
    let script = derive.addr.script_pubkey();

    loop {
        let r = client.scripthash_txs(&script, last_seen).await?;
        match &r[..] {
            [a @ .., esplora::Tx { txid, .. }] if a.len() >= PAGE_SIZE - 1 => {
                last_seen = Some(*txid);
                res.extend(r);
            }
            _ => {
                res.extend(r);
                break;
            }
        }
    }
    Ok(res)
}
