/// Fetch actived tasks from blockchains.
/// If the given chain is EVM based, fetch tasks from solidity-based smart contract storage through RPC request.
/// For example, call RPC methods defined here:
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L70
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L74
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

/// Call method `claim` of contract/pallet through RPC to claim the actived tasks
/// For example, call RPC method defined here:
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L63
///
/// Return account nonce that related to this transaction
struct TaskClaimer;
impl TaskClaimer {
    pub fn claim_task(chain: &Vec<u8>, task_id: &TaskId) -> Result<u64, Error> {
        Err(Error::Unimplemented)
    }
}
