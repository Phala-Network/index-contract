use alloc::{string::String, vec::Vec};
use index::graph::{Chain, ChainType};
use scale::{Decode, Encode};

use super::account::AccountInfo;
use super::bridge::BridgeStep;
use super::context::Context;
use super::step::{Step, StepMeta};
use super::swap::SwapStep;
use super::task::{Task, TaskId};
use super::traits::Runner;
use hex_literal::hex;
use pink_web3::{
    api::{Eth, Namespace},
    contract::{tokens::Detokenize, Contract, Error as PinkError, Options},
    ethabi::Token,
    transports::{resolve_ready, PinkHttp},
    types::Address,
};
use primitive_types::{H160, U256};
use serde::Deserialize;

/// Call method `claim` of contract/pallet through RPC to claim the actived tasks
/// For example, call RPC method defined here:
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L63
///
/// Return account nonce that related to this transaction
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ClaimStep {
    /// Chain name
    pub chain: String,
    /// Task Id
    pub id: TaskId,
}

impl Runner for ClaimStep {
    fn runnable(&self) -> bool {
        // TODO: implement
        true
    }

    fn run(&self, context: &Context) -> Result<(), &'static str> {
        let signer = context.signer;
        let chain = context
            .graph
            .get_chain(self.chain.clone())
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;

        match chain.chain_type {
            ChainType::Evm => Ok(self.claim_evm_actived_tasks(chain, self.id, &signer)?),
            ChainType::Sub => Err("Unimplemented"),
        }
    }

    fn check(&self, _nonce: u64) -> bool {
        // TODO: implement
        false
    }
}

impl ClaimStep {
    fn claim_evm_actived_tasks(
        &self,
        _chain: Chain,
        _task_id: TaskId,
        _worker_key: &[u8; 32],
    ) -> Result<(), &'static str> {
        Err("Unimplemented")
    }
}

/// Fetch actived requests from blockchains and construct a `Task` from it.
/// If the given chain is EVM based, fetch requests from solidity-based smart contract storage through RPC request.
/// For example, call RPC methods defined here:
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L70
///     https://github.com/Phala-Network/index-solidity/blob/07584ede4d6631c97dabc9ba52509c36d4fceb5b/contracts/Aggregator.sol#L74
/// If the given chain is Substrate based, fetch requests from pallet storage through RPC request.
pub struct ActivedTaskFetcher {
    chain: Chain,
    worker: AccountInfo,
}
impl ActivedTaskFetcher {
    pub fn new(chain: Chain, worker: AccountInfo) -> Self {
        ActivedTaskFetcher { chain, worker }
    }

    pub fn fetch_task(&self) -> Result<Task, &'static str> {
        match self.chain.chain_type {
            ChainType::Evm => Ok(self.query_evm_actived_request(&self.chain, &self.worker)?),
            ChainType::Sub => Err("Unimplemented"),
        }
    }

    fn query_evm_actived_request(
        &self,
        chain: &Chain,
        worker: &AccountInfo,
    ) -> Result<Task, &'static str> {
        let handler_on_goerli: H160 = hex!("bEA1C40ecf9c4603ec25264860B9b6623Ff733F5").into();
        let transport = Eth::new(PinkHttp::new(&chain.endpoint));
        let handler = Contract::from_json(
            transport,
            // TODO: use handler from ChainInfo
            handler_on_goerli,
            include_bytes!("./abi/handler.json"),
        )
        .map_err(|_| "ConstructContractFailed")?;
        let worker_address: Address = worker.account20.into();
        let request_id: [u8; 32] = resolve_ready(handler.query(
            "getLastActivedRequest",
            worker_address,
            None,
            Options::default(),
            None,
        ))
        .unwrap();
        let deposit_data: DepositData = resolve_ready(handler.query(
            "getRequestData",
            request_id.clone(),
            None,
            Options::default(),
            None,
        ))
        .unwrap();
        println!("deposit data: {:?}", &deposit_data);
        deposit_data.to_task(&chain.name, request_id)
    }
}

// Define the structures to parse deposit data json
#[derive(Debug)]
struct DepositData {
    sender: Address,
    token: Address,
    amount: U256,
    recipient: Vec<u8>,
    request: String,
}

impl Detokenize for DepositData {
    fn from_tokens(tokens: Vec<Token>) -> std::result::Result<Self, PinkError>
    where
        Self: Sized,
    {
        if tokens.len() == 1 {
            let deposit_raw = tokens[0].clone();
            match deposit_raw {
                Token::Tuple(deposit_data) => {
                    match (
                        deposit_data[0].clone(),
                        deposit_data[1].clone(),
                        deposit_data[2].clone(),
                        deposit_data[3].clone(),
                        deposit_data[4].clone(),
                    ) {
                        (
                            Token::Address(sender),
                            Token::Address(token),
                            Token::Uint(amount),
                            Token::Bytes(recipient),
                            Token::String(request),
                        ) => Ok(DepositData {
                            sender,
                            token,
                            amount,
                            recipient,
                            request,
                        }),
                        _ => Err(PinkError::InvalidOutputType(String::from(
                            "Return type dismatch",
                        ))),
                    }
                }
                _ => Err(PinkError::InvalidOutputType(String::from(
                    "Unexpected output type",
                ))),
            }
        } else {
            Err(PinkError::InvalidOutputType(String::from("Invalid length")))
        }
    }
}

impl DepositData {
    fn to_task(&self, source_chain: &String, id: [u8; 32]) -> Result<Task, &'static str> {
        let mut uninitialized_task: Task = Default::default();
        println!("request data: {:?}", &self.request);
        let request_data_json: RequestData =
            pink_json::from_str(&self.request).map_err(|_| "Request data parse failed")?;
        uninitialized_task.id = id.into();
        // Preset
        uninitialized_task.source = source_chain.clone();
        uninitialized_task.sender = self.sender.as_bytes().into();
        uninitialized_task.recipient = self.recipient.clone();
        // Insert claim step
        uninitialized_task.steps.push(Step {
            meta: StepMeta::Claim(ClaimStep {
                chain: source_chain.clone(),
                id: id.into(),
            }),
            chain: source_chain.clone(),
            nonce: None,
        });
        for op in request_data_json.op.iter() {
            match op {
                Operation::Swap(swap_op) => {
                    uninitialized_task.steps.push(Step {
                        meta: StepMeta::Swap(SwapStep {
                            spend_asset: swap_op.data.spend_asset.as_bytes().into(),
                            receive_asset: swap_op.data.receive_asset.as_bytes().into(),
                            chain: swap_op.data.chain.clone(),
                            dex: swap_op.data.dex.clone(),
                            cap: swap_op.data.cap,
                            flow: swap_op.data.flow,
                            impact: swap_op.data.impact,
                            b0: swap_op.data.b0,
                            b1: swap_op.data.b1,
                            spend: swap_op.data.spend,
                        }),
                        chain: swap_op.data.chain.clone(),
                        nonce: None,
                    });
                }
                Operation::Bridge(bridge_op) => uninitialized_task.steps.push(Step {
                    meta: StepMeta::Bridge(BridgeStep {
                        from: bridge_op.data.from.as_bytes().into(),
                        source_chain: bridge_op.data.source_chain.clone(),
                        to: bridge_op.data.to.as_bytes().into(),
                        dest_chain: bridge_op.data.dest_chain.clone(),
                        fee: bridge_op.data.fee,
                        cap: bridge_op.data.cap,
                        flow: bridge_op.data.flow,
                        b0: bridge_op.data.b0,
                        b1: bridge_op.data.b1,
                        amount: bridge_op.data.amount,
                    }),
                    chain: bridge_op.data.source_chain.clone(),
                    nonce: None,
                }),
            }
        }

        Ok(uninitialized_task)
    }
}

#[derive(Deserialize)]
struct RequestData {
    op: Vec<Operation>,
}

#[derive(Deserialize)]
enum Operation {
    Swap(SwapOperation),
    Bridge(BridgeOperation),
}

#[derive(Deserialize)]
struct SwapOperation {
    op_type: String,
    data: SwapOperationData,
}

#[derive(Deserialize)]
struct SwapOperationData {
    spend_asset: Address,
    receive_asset: Address,
    chain: String,
    dex: String,
    cap: u128,
    flow: u128,
    impact: u128,
    b0: Option<u128>,
    b1: Option<u128>,
    spend: u128,
}

#[derive(Deserialize)]
struct BridgeOperation {
    op_type: String,
    data: BridgeOperationData,
}

#[derive(Deserialize)]
struct BridgeOperationData {
    from: Address,
    source_chain: String,
    to: Address,
    dest_chain: String,
    fee: u128,
    cap: u128,
    flow: u128,
    b0: Option<u128>,
    b1: Option<u128>,
    amount: u128,
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use hex_literal::hex;

    #[test]
    fn test_fetch_task_from_evm() {
        dotenv().ok();

        pink_extension_runtime::mock_ext::mock_all_ext();

        let worker_address: H160 = hex!("f60dB2d02af3f650798b59CB6D453b78f2C1BC90").into();
        let task = ActivedTaskFetcher {
            chain: Chain {
                id: 0,
                name: String::from("Ethereum"),
                chain_type: ChainType::Evm,
                endpoint: String::from(
                    "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
                ),
            },
            worker: AccountInfo {
                account20: worker_address.into(),
                account32: [0; 32],
            },
        }
        .fetch_task()
        .unwrap();
        println!("Evm task: {:?}", &task);
        assert_eq!(task.steps.len(), 3);
    }
}
