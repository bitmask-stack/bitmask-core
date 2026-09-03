use bpwallet::{MiningInfo, TxStatus, Txid};
use esplora::{AsyncClient, Error};
use rgbcore::validation::{ResolveWitness, WitnessResolverError, WitnessStatus};
use rgbcore::vm::{WitnessOrd, WitnessPos};
use rgbstd::containers::{PubWitness, Transfer};
use std::collections::HashMap;
use std::num::NonZeroU32;

use rgbcore::ChainNet;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum ResolverError {
    #[from]
    #[display(inner)]
    Error(Error),

    /// invalid anchor {0}
    InvalidAnchor(String),

    /// cannot be load {0}
    Prefetch(String),

    /// no block height found on response
    NoBlockHeight,

    /// no block hash found on response
    NoBlockHash,

    /// block height is not non zero
    NoNonZero,

    /// Liquid is not yet supported
    LiquidTx,
}

#[derive(Clone, Default, Debug)]
pub struct BitmaskResolver {
    pub explorer_url: String,
    pub witnesses: HashMap<Txid, PubWitness>,
    pub witnesses_status: HashMap<Txid, TxStatus>,
}

impl BitmaskResolver {
    #[allow(clippy::result_large_err)]
    pub fn explorer(&self) -> Result<AsyncClient, ResolverError> {
        Ok(esplora::Builder::new(&self.explorer_url).build_async()?)
    }

    pub async fn tx_status(&self, txid: &Txid) -> Result<TxStatus, ResolverError> {
        let client = self.clone().explorer()?;
        let resp = client.tx_status(txid).await?;
        let Some(hash) = resp.block_hash else {
            return Ok(TxStatus::Mempool);
        };
        Ok(if resp.confirmed {
            let height =
                NonZeroU32::new(resp.block_height.unwrap_or_default()).unwrap_or(NonZeroU32::MIN);

            TxStatus::Mined(MiningInfo {
                height,
                time: resp.block_time.unwrap_or_default(),
                block_hash: hash,
            })
        } else {
            TxStatus::Mempool
        })
    }
}

impl ResolveWitness for BitmaskResolver {
    #[cfg(target_arch = "wasm32")]
    fn resolve_witness(
        &self,
        witness_id: rgbcore::Txid,
    ) -> Result<WitnessStatus, WitnessResolverError> {
        let bp_id = crate::rgb::compat::txid_bitcoin_to_bp(witness_id);
        if self.witnesses.get(&bp_id).is_none() {
            return Ok(WitnessStatus::Unresolved);
        };

        if let Some(status) = self.witnesses_status.get(&bp_id) {
            let status = match status {
                TxStatus::Mined(info) => {
                    let pos = WitnessPos::bitcoin(info.height, info.time as i64).unwrap();
                    WitnessOrd::Mined(pos)
                }
                _ => WitnessOrd::Tentative,
            };

            let witness = self.witnesses.get(&bp_id).unwrap();
            Ok(WitnessStatus::Resolved(
                witness.tx().unwrap().clone(),
                status,
            ))
        } else {
            Ok(WitnessStatus::Unresolved)
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_witness(
        &self,
        witness_id: rgbcore::Txid,
    ) -> Result<WitnessStatus, WitnessResolverError> {
        let bp_id = crate::rgb::compat::txid_bitcoin_to_bp(witness_id);
        let esplora_client = esplora::Builder::new(&self.explorer_url)
            .build_blocking()
            .map_err(|e| WitnessResolverError::ResolverIssue(Some(witness_id), e.to_string()))?;

        let tx = esplora_client
            .tx(&bp_id)
            .map_err(|e| WitnessResolverError::ResolverIssue(Some(witness_id), e.to_string()))?;

        let status = esplora_client
            .tx_status(&bp_id)
            .map_err(|e| WitnessResolverError::ResolverIssue(Some(witness_id), e.to_string()))?;
        let ord = match status.block_height.zip(status.block_time) {
            Some((h, t)) => {
                let height = NonZeroU32::new(h).ok_or(WitnessResolverError::InvalidResolverData)?;
                WitnessOrd::Mined(
                    WitnessPos::bitcoin(height, t as i64)
                        .ok_or(WitnessResolverError::InvalidResolverData)?,
                )
            }
            None => WitnessOrd::Tentative,
        };

        Ok(WitnessStatus::Resolved(
            crate::rgb::compat::tx_bp_to_bitcoin(tx.unwrap()),
            ord,
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn check_chain_net(&self, chain_net: ChainNet) -> Result<(), WitnessResolverError> {
        let esplora_client = esplora::Builder::new(&self.explorer_url)
            .build_blocking()
            .map_err(|_| WitnessResolverError::WrongChainNet)?;

        let block_hash = esplora_client
            .block_hash(0)
            .map_err(|_| WitnessResolverError::WrongChainNet)?;
        let genesis = chain_net.chain_hash();
        if AsRef::<[u8]>::as_ref(&genesis) != AsRef::<[u8]>::as_ref(block_hash.as_ref()) {
            return Err(WitnessResolverError::WrongChainNet);
        }
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn check_chain_net(&self, _chain_net: ChainNet) -> Result<(), WitnessResolverError> {
        Ok(())
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, From, Error, Display)]
#[display(doc_comments)]
pub enum PrefetchError {
    /// esplora error. {0}
    EsploraError(#[from] Error),
    /// broken logic error.
    BrokenLogicError,
    /// try from int error. {0}
    TryFromIntError(#[from] std::num::TryFromIntError),
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn prefetch_transfer(_: &Transfer, _: &mut BitmaskResolver) -> Result<(), PrefetchError> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn esplora_tx_status_to_bp(tx_status: esplora::TxStatus) -> TxStatus {
    if tx_status.confirmed {
        TxStatus::Mined(MiningInfo {
            height: NonZeroU32::new(tx_status.block_height.unwrap_or(0)).unwrap(),
            time: tx_status.block_time.unwrap_or(0),
            block_hash: tx_status.block_hash.unwrap(),
        })
    } else {
        TxStatus::Mempool
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn prefetch_transfer(
    transfer: &Transfer,
    resolver: &mut BitmaskResolver,
) -> Result<(), PrefetchError> {
    let esplora_client = esplora::Builder::new(&resolver.explorer_url).build_async()?;
    for bundle in transfer.bundles.clone() {
        match bundle.pub_witness {
            PubWitness::Txid(txid) => {
                let bp_id = crate::rgb::compat::txid_bitcoin_to_bp(txid);
                let find_tx = esplora_client
                    .tx(&bp_id)
                    .await
                    .map_err(PrefetchError::EsploraError)?;

                let tx_status = esplora_client
                    .tx_status(&bp_id)
                    .await
                    .map_err(PrefetchError::EsploraError);

                let tx_status = if let Ok(tx_status) = tx_status {
                    esplora_tx_status_to_bp(tx_status)
                } else {
                    TxStatus::Unknown
                };

                if let Some(tx) = find_tx {
                    let pub_witness = PubWitness::with(crate::rgb::compat::tx_bp_to_bitcoin(tx));
                    resolver.witnesses.insert(bp_id, pub_witness);
                    resolver.witnesses_status.insert(bp_id, tx_status);
                };
            }
            PubWitness::Tx(tx) => {
                let bp_id = crate::rgb::compat::txid_bitcoin_to_bp(tx.compute_txid());
                let pub_witness = PubWitness::Tx(tx);
                resolver.witnesses.insert(bp_id, pub_witness);
                resolver.witnesses_status.insert(bp_id, TxStatus::Mempool);
            }
        }
    }
    Ok(())
}
