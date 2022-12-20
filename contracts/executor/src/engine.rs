

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use pink_extension::EcdhPublicKey;
use index::prelude::*;
use index::prelude::ChainInfo;
use pallet_index::types::{StepStatus, StepMeta, Step, Task, TaskId};
use index_registry::RegistryRef;
use super::account::AccountInfo;
use super::claimer::TaskClaimer;

/// Check transaction result on specific `chain` with given transaction nonce.
/// Retuen true if transaction execute successfully (e.g. packed in block)
/// 
/// Different chain have diffent way to check transaction result. For instance,
/// We can use RPC `get_transaction` on Ethereum to check the transaction status
struct ExecutionChecker;
impl ExecutionChecker {
    pub fn check_execution(step: Step, account: AccountInfo) -> Result<bool, Error> {
        let source_chain = get_chain(step);
        let expected_nonce = step.nonce;
        let current_nonce = self.get_onchain_nonce(source_chain, account)?;
        if current_nonce >= expected_nonce {
            // Transaction executed, check result according specific execution type
            match step.meta {
                Claim(claim_step) => {
                    match source_chain {
                        Evm(evm_chain) => {
                            // Lookup if task is exist on onchain actived list,
                            // if not exist, means it was claimed successfully.
                            self.evm_lookup_task(source_chain, step.meta.id)?.is_none()
                        },
                        Sub(sub_chain) => {

                        }
                    }
                },
                Swap(swap_step) => {
                    let (chain, spend_asset, receive_asset, spend) = ParseArgs(swap_step);
                    let balance_of_spend_asset = self.get_balance(chain, account)?;
                    // Maybe have some transaction fee to pay
                    if balance_of_spend_asset <= swap_step.b0 - spend {
                        true
                    } else {
                        false
                    }
                },
                Bridge(bridge_step) => {
                    let (src_chain, src_asset, dest_chain, dest_asset, amount) = ParseArgs(bridge_step);

                    let balance_on_source_chain = self.get_balance(src_chain, account)?;
                    let balance_on_dest_chain = self.get_balance(dest_chain, account)?;
                    // Maybe have some transaction fee to pay
                    if (balance_on_source_chain <= bridge_step.b0 - bridge_step.amount) && 
                    (balance_on_dest_chain == (bridge_step.b1 + bridge_step.amount - bridge_step.fee)){
                        true
                    } else {
                        false
                    }
                },
                _ => true
            }
        } else {
            false
        }
    }

    fn get_onchain_nonce(chain: ChainInfo, account: AccountInfo) -> Result<u64, Error> {
        Err(Error::Unimplemented)
    }

    // Lookup handler contract `actived_tasks` queue
    fn evm_lookup_task(chain: ChainInfo, id: TaskId) -> Result<Option<Task>, Error> {
        Err(Error::Unimplemented)
    }

    fn get_balance(chain: ChainInfo, account: AccountInfo) -> Result<u128, Error> {
        Err(Error::Unimplemented)
    }
}

struct StepExecutor(RegistryRef);
impl StepExecutor {
    /// Execute step according to step type, return corresponding account nonce if success.
    pub fn execute_step(signer: &[u8; 32], step: &Step) -> Result<u64, Error> {
        match step.meta {
            Claim(claim_step) => {
                let chain = get_chain(claim_step.chain);
                TaskClaimer::claim_task(chain, step, signer)
            },
            Begin(begin_step) => {
                // ingore
            },
            End(end_step) => {
                // ingore
            },
            Swap(swap_step) => {
                let (chain, spend_asset, receive_asset, spend) = ParseArgs(swap_step);
                // Get executor according to `chain` from registry
                let executor = self.0.dex_executors.get(&chain).ok_or(Error::ExecuteFailed)?;
                let recipient = if chain.is_evm() {
                    AccountInfo::from(signer).account20
                } else {
                    AccountInfo::from(signer).account32
                };
                // Do swap operation
                <executor as DexExecutor>::swap(signer, spend_asset, receive_asset, spend, recipient)
            },
            Bridge(bridge_step) => {
                let (src_chain, src_asset, dest_chain, amount) = ParseArgs(bridge_step);

                // Get executor according to `src_chain` and `des_chain`
                let executor = self.0.bridge_executors.get(&[src_chain, dest_chain].concat()).ok_or(Error::ExecuteFailed)?;
                let recipient = if chain.is_evm() {
                    AccountInfo::from(signer).account20
                } else {
                    AccountInfo::from(signer).account32
                };
                // Do bridge transfer operation
                <executor as BridgeExecutor>::transfer(signer, src_asset, recipient, amount)
            }
        }
    }
}