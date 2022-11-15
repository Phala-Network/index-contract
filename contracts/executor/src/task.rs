

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use pink_extension::EcdhPublicKey;
use index::prelude::*;
use index::prelude::ChainInfo;
use pallet_index::types::{EdgeStatus, EdgeMeta, Edge, Task, TaskId};

/// Fetch actived tasks from blockchains.
/// If the given chain is EVM based, fetch tasks from solidity-based smart contract storage through RPC request.
/// If the given chain is Substrate based, fetch tasks from pallet storage through RPC request.
struct ActivedTaskFetcher {
    chain: ChainInfo,
    worker: EcdhPublicKey,
}
impl ActivedTaskFetcher {
    pub fn new(chain: ChainInfo, worker: EcdhPublicKey) -> Self {
        ActivedTaskFetcher { chain, worker }
    }

    pub fn fetch_tasks() -> Result<Task, Error> {

        Err(Error::Unimplemented)
    }
}

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
    pub fn check_transaction(chain: Vec<u8>, hash: Vec<u8>) -> Result<bool, Error> {
        Err(Error::Unimplemented)
    }
}

/// Call method `claim` of contract/pallet through RPC to claim the actived tasks
///
/// Return transaction hash if success.
struct TaskClaimer;
impl TaskClaimer {
    pub fn claim_task(chain: &Vec<u8>, task_id: &TaskId) -> Result<Vec<u8>, Error> {
        Err(Error::Unimplemented)
    }
}

/// Call method `update_task` of pallet-index:
/// https://github.com/Phala-Network/khala-parachain/blob/a0585857d86d9b1a63bbfab57d695eac5c8e3259/pallets/index/src/lib.rs#L93
///
/// Return transaction hash if success.
struct TaskUploader;
impl TaskUploader {
    pub fn upload_task(worker: &AccountId, task: &Task) -> Result<Vec<u8>, Error> {
        Err(Error::Unimplemented)
    }
}

struct Step;
impl Step {
    /// Execute step according to edge type, return corresponding transaction hash if success.
    pub fn execute_step(edge: &Edge) -> Result<Vec<u8>, Error> {
        match edge.edge {
            Source(source_edge) => {
                // ingore
            },
            Sink(sink_edge) => {

            },
            Swap(swap_edge) => {
                let (chain, spend_asset, receive_asset, amount) = ParseArgs(swap_edge);
                // index::DexExecutor::swap(...)?;
            },
            Bridge(bridge_edge) => {
                let (src_chain, src_asset, dest_chain, dest_asset, amount) = ParseArgs(bridge_edge);
                // index::BridgeExecutor::transfer(...)?;
            }
        }
        Err(Error::Unimplemented)
    }

    /// Revert step according to edge type, return corresponding transaction hash if success.
    pub fn revert_step(edge: &Edge) -> Result<Vec<u8>, Error> {
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