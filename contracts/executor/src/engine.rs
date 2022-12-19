

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use pink_extension::EcdhPublicKey;
use index::prelude::*;
use index::prelude::ChainInfo;
use pallet_index::types::{StepStatus, StepMeta, Step, Task, TaskId};
use index_registry::RegistryRef;
use super::account::AccountInfo;

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
struct ExecutionChecker;
impl ExecutionChecker {
    pub fn check_execution(chain: Step, account: AccountInfo) -> Result<bool, Error> {
        // Check transaction result according to different step type
        Err(Error::Unimplemented)
    }
}

struct StepExecutor(RegistryRef);
impl StepExecutor {
    /// Execute step according to step type, return corresponding account nonce if success.
    pub fn execute_step(signer: &[u8; 32], step: &Step) -> Result<u64, Error> {
        match step.step {
            Claim(claim_step) => {
                // TODO: claim task from source chain
            },
            Begin(begin_step) => {
                // ingore
            },
            End(end_step) => {

            },
            Swap(swap_step) => {
                let (chain, spend_asset, receive_asset, amount) = ParseArgs(swap_step);
                
                // Get executor according to `chain` from registry
                let executor = self.0.dex_executors.get(&chain).ok_or(Error::ExecuteFailed)?;

                // Do swap operation
                <executor as DexExecutor>::swap(...);
            },
            Bridge(bridge_step) => {
                let (src_chain, src_asset, dest_chain, dest_asset, amount) = ParseArgs(bridge_step);
                
                // Get executor according to `src_chain` and `des_chain`
                let executor = self.0.bridge_executors.get(&[src_chain, dest_chain].concat()).ok_or(Error::ExecuteFailed)?;

                // Do bridge transfer operation
                <executor as BridgeExecutor>::transfer(...);
            }
        }
        Err(Error::Unimplemented)
    }

    /// Parse source chain from `step`
    pub fn source_chain(step: &Step) -> Result<Vec<u8>, Error> {

    }

    /// Parse dest chain from `step`
    pub fn dest_chain(step: &Step) -> Result<Vec<u8>, Error> {

    }
}