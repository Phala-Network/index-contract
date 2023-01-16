use alloc::{string::String, vec::Vec};
use index::graph::{Chain, ChainType, NonceFetcher};
use scale::{Decode, Encode};

use super::account::AccountInfo;
use super::bridge::BridgeStep;
use super::context::Context;
use super::step::{Step, StepMeta};
use super::swap::SwapStep;
use super::task::{OnchainTasks, Task, TaskId};
use super::traits::Runner;
use hex_literal::hex;
use pink_web3::{
    api::{Eth, Namespace},
    contract::{tokens::Detokenize, Contract, Error as PinkError, Options},
    ethabi::Token,
    keys::pink::KeyPair,
    signing::Key,
    transports::{resolve_ready, PinkHttp},
    types::Address,
};

use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;
use primitive_types::{H160, U256};
use serde::Deserialize;

/// Call method `claim` of contract/pallet through RPC to claim the actived tasks
/// For example, call RPC method defined here:
///     https://github.com/Phala-Network/index-solidity/blob/0a1efe4b228185a37635dd872e1130eb3564ef6a/contracts/Handler.sol#L108
///
/// Return account nonce that related to this transaction
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ClaimStep {
    /// Chain name
    pub chain: String,
    /// Task Id
    pub id: TaskId,
    /// Asset that will transfer to worker account during claim
    pub asset: Vec<u8>,
    /// Original relayer account balance of received asset
    pub b0: Option<u128>,
}

impl Runner for ClaimStep {
    fn runnable(
        &self,
        _nonce: u64,
        _context: &Context,
        client: Option<&mut SubstrateRollupClient>,
    ) -> Result<bool, &'static str> {
        // If task already exist in rollup storage, it is ready to be claimed
        Ok(OnchainTasks::lookup_task(client.ok_or("MissingClient")?, &self.id).is_some())
    }

    fn run(&self, nonce: u64, context: &Context) -> Result<(), &'static str> {
        let signer = context.signer;
        let chain = context
            .graph
            .get_chain(self.chain.clone())
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;

        match chain.chain_type {
            ChainType::Evm => Ok(self.claim_evm_actived_tasks(chain, self.id, &signer, nonce)?),
            ChainType::Sub => Err("Unimplemented"),
        }
    }

    fn check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        let worker = KeyPair::from(context.signer);
        // Check if the transaction has been executed
        let chain = context
            .graph
            .get_chain(self.chain.clone())
            .ok_or("MissingChain")?;
        let onchain_nonce = chain
            .get_nonce(worker.address().as_bytes().into())
            .map_err(|_| "FetchNonceFailed")?;
        Ok((onchain_nonce - nonce) == 1)

        // TODO: Check if the transaction is successed or not
    }

    fn sync_check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        self.check(nonce, context)
    }
}

impl ClaimStep {
    fn claim_evm_actived_tasks(
        &self,
        chain: Chain,
        task_id: TaskId,
        worker_key: &[u8; 32],
        nonce: u64,
    ) -> Result<(), &'static str> {
        // TODO: use handler configed in `chain`
        let handler_on_goerli: H160 = hex!("Bf30B9BD94C584d8449fDE4fa57F46c838b62dc2").into();
        let transport = Eth::new(PinkHttp::new(chain.endpoint));
        let handler = Contract::from_json(
            transport,
            handler_on_goerli,
            include_bytes!("./abi/handler.json"),
        )
        .map_err(|_| "ConstructContractFailed")?;
        let worker = KeyPair::from(*worker_key);

        // Estiamte gas before submission
        let gas = resolve_ready(handler.estimate_gas(
            "claim",
            task_id,
            worker.address(),
            Options::default(),
        ))
        .map_err(|_| "GasEstimateFailed")?;

        // Submit the claim transaction
        let _tx_id = resolve_ready(handler.signed_call(
            "claim",
            task_id,
            Options::with(|opt| {
                opt.gas = Some(gas);
                opt.nonce = Some(nonce.into());
            }),
            worker,
        ))
        .map_err(|_| "ClaimSubmitFailed")?;

        Ok(())
    }
}

/// Fetch actived requests from blockchains and construct a `Task` from it.
/// If the given chain is EVM based, fetch requests from solidity-based smart contract storage through RPC request.
/// For example, call RPC methods defined here:
///     https://github.com/Phala-Network/index-solidity/blob/7b4458f9b8277df8a1c705a4d0f264476f42fcf2/contracts/Handler.sol#L147
///     https://github.com/Phala-Network/index-solidity/blob/7b4458f9b8277df8a1c705a4d0f264476f42fcf2/contracts/Handler.sol#L165
/// If the given chain is Substrate based, fetch requests from pallet storage through RPC request.
pub struct ActivedTaskFetcher {
    pub chain: Chain,
    pub executor: AccountInfo,
}
impl ActivedTaskFetcher {
    pub fn new(chain: Chain, executor: AccountInfo) -> Self {
        ActivedTaskFetcher { chain, executor }
    }

    pub fn fetch_task(&self) -> Result<Task, &'static str> {
        match self.chain.chain_type {
            ChainType::Evm => Ok(self.query_evm_actived_request(&self.chain, &self.executor)?),
            ChainType::Sub => Err("Unimplemented"),
        }
    }

    fn query_evm_actived_request(
        &self,
        chain: &Chain,
        worker: &AccountInfo,
    ) -> Result<Task, &'static str> {
        // TODO: use handler configed in `chain`
        let handler_on_goerli: H160 = hex!("bEA1C40ecf9c4603ec25264860B9b6623Ff733F5").into();
        let transport = Eth::new(PinkHttp::new(&chain.endpoint));
        let handler = Contract::from_json(
            transport,
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
            request_id,
            None,
            Options::default(),
            None,
        ))
        .unwrap();
        deposit_data.to_task(&chain.name, request_id)
    }
}

// Define the structures to parse deposit data json
#[allow(dead_code)]
#[derive(Debug)]
struct DepositData {
    sender: Address,
    token: Address,
    amount: U256,
    recipient: Vec<u8>,
    request: String,
}

impl Detokenize for DepositData {
    fn from_tokens(tokens: Vec<Token>) -> Result<Self, PinkError>
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
    fn to_task(&self, source_chain: &str, id: [u8; 32]) -> Result<Task, &'static str> {
        let request_data_json: RequestDataJson = pink_json::from_str(&self.request).unwrap();
        if request_data_json.is_empty() {
            return Err("EmptyTask");
        }

        let mut uninitialized_task = Task {
            id,
            source: source_chain.into(),
            sender: self.sender.as_bytes().into(),
            recipient: self.recipient.clone(),
            ..Default::default()
        };

        // Insert claim step
        uninitialized_task.steps.push(Step {
            meta: StepMeta::Claim(ClaimStep {
                chain: source_chain.into(),
                id,
                asset: request_data_json[0].spend_asset.as_bytes().into(),
                b0: None,
            }),
            chain: source_chain.into(),
            nonce: None,
        });

        for op in request_data_json.iter() {
            if op.op_type == *"swap" {
                uninitialized_task.steps.push(Step {
                    meta: StepMeta::Swap(SwapStep {
                        spend_asset: op.spend_asset.as_bytes().into(),
                        receive_asset: op.receive_asset.as_bytes().into(),
                        chain: op.source_chain.clone(),
                        dex: op.dex.clone(),
                        cap: self.u128_from_string(&op.cap)?,
                        flow: self.u128_from_string(&op.flow)?,
                        impact: self.u128_from_string(&op.impact)?,
                        b0: None,
                        b1: None,
                        spend: self.u128_from_string(&op.spend)?,
                    }),
                    chain: op.source_chain.clone(),
                    nonce: None,
                });
            } else if op.op_type == *"bridge" {
                uninitialized_task.steps.push(Step {
                    meta: StepMeta::Bridge(BridgeStep {
                        from: op.spend_asset.as_bytes().into(),
                        source_chain: op.source_chain.clone(),
                        to: op.receive_asset.as_bytes().into(),
                        dest_chain: op.dest_chain.clone(),
                        fee: self.u128_from_string(&op.fee)?,
                        cap: self.u128_from_string(&op.cap)?,
                        flow: self.u128_from_string(&op.flow)?,
                        b0: None,
                        b1: None,
                        amount: self.u128_from_string(&op.spend)?,
                    }),
                    chain: op.source_chain.clone(),
                    nonce: None,
                })
            } else {
                return Err("Unrecognized op type");
            }
        }

        Ok(uninitialized_task)
    }

    fn u128_from_string(&self, amount: &str) -> Result<u128, &'static str> {
        use fixed::types::U128F0 as Fp;
        let fixed_u128 = Fp::from_str(amount).or(Err("U128ConversionFailed"))?;
        Ok(fixed_u128.to_num())
    }
}

type RequestDataJson = Vec<OperationJson>;

#[derive(Deserialize)]
struct OperationJson {
    op_type: String,
    source_chain: String,
    dest_chain: String,
    spend_asset: Address,
    receive_asset: Address,
    dex: String,
    fee: String,
    cap: String,
    flow: String,
    impact: String,
    spend: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use alloc::vec;
    use dotenv::dotenv;
    use hex_literal::hex;
    use index::{
        graph::{Chain, ChainType, Graph},
        utils::ToArray,
    };

    #[test]
    fn test_fetch_task_from_evm() {
        dotenv().ok();

        pink_extension_runtime::mock_ext::mock_all_ext();

        let executor_address: H160 = hex!("f60dB2d02af3f650798b59CB6D453b78f2C1BC90").into();
        let task = ActivedTaskFetcher {
            chain: Chain {
                id: 0,
                name: String::from("Ethereum"),
                chain_type: ChainType::Evm,
                endpoint: String::from(
                    "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
                ),
                native_asset: vec![0],
                foreign_asset: None,
            },
            executor: AccountInfo {
                account20: executor_address.into(),
                account32: [0; 32],
            },
        }
        .fetch_task()
        .unwrap();
        assert_eq!(task.steps.len(), 3);
        match (
            task.steps[0].meta.clone(),
            task.steps[1].meta.clone(),
            task.steps[2].meta.clone(),
        ) {
            (
                StepMeta::Claim(claim_step),
                StepMeta::Swap(swap_meta),
                StepMeta::Bridge(bridge_meta),
            ) => {
                assert_eq!(claim_step.chain, String::from("Ethereum"));
                assert_eq!(swap_meta.spend, 100_000_000_000_000_000_000 as u128);
                assert_eq!(bridge_meta.amount, 12_000_000);
            }
            _ => assert!(false),
        }
    }

    #[test]
    #[ignore]
    fn test_claim_task_from_evm_chain() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        // This key is just for test, never put real money in it.
        let mock_worker_key: [u8; 32] =
            hex::decode("994efb9f9df9af03ad27553744a6492bfd8d1b22aa203769e75e200043110a48")
                .unwrap()
                .to_array();
        // Current transaction count of the mock worker account
        let nonce = 0;
        let goerli = Chain {
            id: 0,
            name: String::from("Goerli"),
            chain_type: ChainType::Evm,
            endpoint: String::from(
                "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
            ),
            native_asset: vec![0],
            foreign_asset: None,
        };

        let claim_step = ClaimStep {
            chain: String::from("Goerli"),
            id: hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap()
                .to_array(),
            asset: hex::decode("B376b0Ee6d8202721838e76376e81eEc0e2FE864").unwrap(),
            b0: None,
        };
        let context = Context {
            signer: mock_worker_key,
            graph: Graph {
                chains: vec![goerli],
                assets: vec![],
                dexs: vec![],
                bridges: vec![],
                dex_pairs: vec![],
                bridge_pairs: vec![],
                dex_indexers: vec![],
            },
            worker_accounts: vec![],
            bridge_executors: vec![],
            dex_executors: vec![],
        };
        // Send claim transaction
        // https://goerli.etherscan.io/tx/0x7a0a6ba48285ffb7c0d00e11ad684aa60b30ac6d4b2cce43c6a0fe3f75791caa
        assert_eq!(claim_step.run(nonce, &context,), Ok(()));

        // Wait 60 seconds to let transaction confirmed
        std::thread::sleep(std::time::Duration::from_millis(60000));

        assert_eq!(claim_step.check(nonce, &context).unwrap(), true);
    }
}
