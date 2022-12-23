use alloc::{string::String, vec::Vec};
use index_registry::types::{ChainInfo, ChainType};
use scale::{Decode, Encode};

use super::account::AccountInfo;
use super::context::Context;
use super::task::{Task, TaskId};
use super::traits::Runner;

/// Call method `claim` of contract/pallet through RPC to claim the actived tasks
/// For example, call RPC method defined here:
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L63
///
/// Return account nonce that related to this transaction
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ClaimStep {
    /// Chain name
    chain: String,
    /// Task Id
    id: TaskId,
}

impl Runner for ClaimStep {
    fn run(&self, context: &Context) -> Result<(), &'static str> {
        let signer = context.signer;
        let chain = context
            .registry
            .get_chain(self.chain.clone())
            .map_err(|_| "MissingChain")?;

        match chain.chain_type {
            ChainType::Evm => Ok(self.claim_evm_actived_tasks(chain, self.id, &signer)?),
            ChainType::Sub => Err("Unimplemented"),
        }
    }

    fn check(&self, nonce: u64) -> bool {
        false
    }
}

impl ClaimStep {
    fn claim_evm_actived_tasks(
        &self,
        chain: ChainInfo,
        task_id: TaskId,
        worker_key: &[u8; 32],
    ) -> Result<(), &'static str> {
        Err("Unimplemented")
    }
}

/// Fetch actived tasks from blockchains.
/// If the given chain is EVM based, fetch tasks from solidity-based smart contract storage through RPC request.
/// For example, call RPC methods defined here:
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L70
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L74
/// If the given chain is Substrate based, fetch tasks from pallet storage through RPC request.
pub struct ActivedTaskFetcher {
    chain: ChainInfo,
    worker: AccountInfo,
}
impl ActivedTaskFetcher {
    pub fn new(chain: ChainInfo, worker: AccountInfo) -> Self {
        ActivedTaskFetcher { chain, worker }
    }

    pub fn fetch_tasks(&self) -> Result<Vec<Task>, &'static str> {
        match self.chain.chain_type {
            ChainType::Evm => {
                Ok(self.query_evm_actived_tasks(self.chain.endpoint.clone(), &self.worker)?)
            }
            ChainType::Sub => Err("Unimplemented"),
        }
    }

    fn query_evm_actived_tasks(
        &self,
        endpoint: String,
        worker: &AccountInfo,
    ) -> Result<Vec<Task>, &'static str> {
        Err("Unimplemented")
    }
}
