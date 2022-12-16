

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use pink_extension::EcdhPublicKey;
use index::prelude::*;
use index::prelude::ChainInfo;
use pallet_index::types::{EdgeStatus, EdgeMeta, Edge, Task, TaskId};
use index_registry::RegistryRef;

/// Fetch runing tasks belong to a specific worker from `pallet-index` on Phala network.
struct RuningTaskFetcher {
    endpoint: Vec<u8>,
    worker: EcdhPublicKey,
}
impl RuningTaskFetcher {
    pub fn new(endpoint: Vec<u8>, worker: EcdhPublicKey) -> Self {
        RuningTaskFetcher { endpoint, worker }
    }

    pub fn fetch_tasks() -> Result<Task, Error> {

        Err(Error::Unimplemented)
    }
}

/// Check transaction result on specific `chain` with given transaction hash.
/// Retuen true if transaction execute successfully (e.g. packed in block)
/// 
/// Different chain have diffent way to check transaction result. For instance,
/// We can use RPC `get_transaction` on Ethereum to check the transaction status
struct TransactionChecker;
impl TransactionChecker {
    pub fn check_transaction(chain: Vec<u8>, nonce: u64) -> Result<bool, Error> {
        // Check transaction result according to different edge type
        Err(Error::Unimplemented)
    }
}

struct Step(RegistryRef);
impl Step {
    /// Execute step according to edge type, return corresponding account nonce if success.
    pub fn execute_step(signer: &[u8; 32], edge: &Edge) -> Result<u64, Error> {
        match edge.edge {
            Source(source_edge) => {
                // ingore
            },
            Sink(sink_edge) => {

            },
            Swap(swap_edge) => {
                let (chain, spend_asset, receive_asset, amount) = ParseArgs(swap_edge);
                
                // Get executor according to `chain` from registry
                let executor = self.0.dex_executors.get(&chain).ok_or(Error::ExecuteFailed)?;

                // Do swap operation
                <executor as DexExecutor>::swap(...);
            },
            Bridge(bridge_edge) => {
                let (src_chain, src_asset, dest_chain, dest_asset, amount) = ParseArgs(bridge_edge);
                
                // Get executor according to `src_chain` and `des_chain`
                let executor = self.0.bridge_executors.get(&[src_chain, dest_chain].concat()).ok_or(Error::ExecuteFailed)?;

                // Do bridge transfer operation
                <executor as BridgeExecutor>::transfer(...);
            }
        }
        Err(Error::Unimplemented)
    }

    /// Revert step according to edge type, return corresponding transaction hash if success.
    pub fn revert_step(signer: &[u8; 32], edge: &Edge) -> Result<Vec<u8>, Error> {
        match edge.edge {
            Source(source_edge) => {

            },
            Sink(sink_edge) => {

            },
            Swap(swap_edge) => {

            },
            Bridge(bridge_edge) => {

            }
        }
        Err(Error::Unimplemented)
    }

    /// Parse source chain from `edge`
    pub fn source_chain(edge: &Edge) -> Result<Vec<u8>, Error> {

    }

    /// Parse dest chain from `edge`
    pub fn dest_chain(edge: &Edge) -> Result<Vec<u8>, Error> {

    }
}