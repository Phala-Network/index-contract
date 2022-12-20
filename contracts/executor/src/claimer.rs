use super::account::AccountInfo;
use super::types::Step;

/// Fetch actived tasks from blockchains.
/// If the given chain is EVM based, fetch tasks from solidity-based smart contract storage through RPC request.
/// For example, call RPC methods defined here:
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L70
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L74
/// If the given chain is Substrate based, fetch tasks from pallet storage through RPC request.
struct ActivedTaskFetcher {
    chain: ChainInfo,
    worker: AccountInfo,
}
impl ActivedTaskFetcher {
    pub fn new(chain: ChainInfo, worker: AccountInfo) -> Self {
        ActivedTaskFetcher { chain, worker }
    }

    pub fn fetch_tasks() -> Result<Vec<Task>, Error> {
        match chain.type {
            Evm => {
                Ok(self.query_evm_actived_tasks(chain.endpoint, worker)?)
            },
            Sub => {

            },
        }
    }

    fn query_evm_actived_tasks(endpoint: Vec<u8>, worker: AccountInfo) -> Result<Vec<Task>, Error> {
        let transport = Eth::new(PinkHttp::new(String::from_utf8_lossy(&endpoint)));
        let handler = Contract::from_json(
            transport,
            chain.handler.unwrap(),
            include_bytes!("./abi/handler.json"),
        )
        .map_err(|_| RegistryError::ConstructContractFailed)?;
        let result: u128 =
            resolve_ready(handler.query("getActivedTasks", worker, None, Options::default(), None))
                .unwrap();
        Ok(result)
    }
}

/// Call method `claim` of contract/pallet through RPC to claim the actived tasks
/// For example, call RPC method defined here:
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L63
///
/// Return account nonce that related to this transaction
struct TaskClaimer;
impl TaskClaimer {
    pub fn claim_task(chain: ChainInfo, step: &Step, worker_key: &[u8; 32]) -> Result<u64, Error> {
        match chain.type {
            Evm => {
                Ok(self.claim_evm_actived_tasks(chain.endpoint, step, worker_key)?)
            },
            Sub => {

            },
        }
    }

    fn claim_evm_actived_tasks(chain: ChainInfo, step: &Step, worker_key: &[u8; 32]) -> Result<Vec<Task>, Error> {
        let transport = Eth::new(PinkHttp::new(String::from_utf8_lossy(&endpoint)));
        let handler = Contract::from_json(
            transport,
            chain.handler.unwrap(),
            include_bytes!("./abi/handler.json"),
        )
        .map_err(|_| RegistryError::ConstructContractFailed)?;
        let signer = KeyPair::from(worker_key);

        // Convert worker EVM address from private key
        let params = (AccountInfo::from(worker_key).account20);
        // Estiamte gas before submission
        let gas = resolve_ready(handler.estimate_gas(
            "claim",
            params.clone(),
            signer.address(),
            Options::default(),
        ))
        .expect("FIXME: failed to estiamte gas");
        let nonce = match step {
            Claim(_) => step.nonce,
            _ => None
        }
        // Actually submit the tx (no guarantee for success)
        let tx_id = resolve_ready(self.contract.signed_call(
            "claim",
            params,
            Options::with(|opt| {
                opt.gas = Some(gas);
                opt.nonce = nonce,
            }),
            signer,
        ))
        .expect("FIXME: submit failed");

        Ok(nonce.unwrap())
    }
}
