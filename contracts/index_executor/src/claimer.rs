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
use xcm::latest::AssetId as XcmAssetId;

use pink_subrpc::{
    get_storage,
    hasher::Twox64Concat,
    storage::{storage_map_prefix, storage_prefix},
};
use pink_web3::{
    api::{Eth, Namespace},
    contract::{tokens::Detokenize, Contract, Error as PinkError, Options},
    ethabi::Token,
    keys::pink::KeyPair,
    signing::Key,
    transports::{resolve_ready, PinkHttp},
    types::{Address, H160, U256},
};

use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;
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
    /// Original worker account balance of received asset
    pub b0: Option<u128>,
}

impl Runner for ClaimStep {
    fn runnable(
        &self,
        nonce: u64,
        context: &Context,
        client: Option<&mut SubstrateRollupClient>,
    ) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);

        // TODO. query off-chain indexer directly get the execution result

        // 1. Check nonce
        let onchain_nonce = worker_account.get_nonce(self.chain.clone(), context)?;
        if onchain_nonce > nonce {
            Ok(false)
        } else {
            // If task already exist in rollup storage, it is ready to be claimed
            Ok(OnchainTasks::lookup_task(client.ok_or("MissingClient")?, &self.id).is_some())
        }
    }

    fn run(&self, nonce: u64, context: &Context) -> Result<Vec<u8>, &'static str> {
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

        // TODO. query off-chain indexer directly get the execution result

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
    ) -> Result<Vec<u8>, &'static str> {
        let handler_on_goerli: H160 = H160::from_slice(&chain.handler_contract);
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
        let tx_id = resolve_ready(handler.signed_call(
            "claim",
            task_id,
            Options::with(|opt| {
                opt.gas = Some(gas);
                opt.nonce = Some(nonce.into());
            }),
            worker,
        ))
        .map_err(|_| "ClaimSubmitFailed")?;
        pink_extension::info!(
            "Submit transaction to claim task {:?} on ${:?}, tx id: {:?}",
            hex::encode(task_id),
            &chain.name,
            hex::encode(tx_id.clone().as_bytes())
        );
        Ok(tx_id.as_bytes().to_vec())
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
    pub worker: AccountInfo,
}
impl ActivedTaskFetcher {
    pub fn new(chain: Chain, worker: AccountInfo) -> Self {
        ActivedTaskFetcher { chain, worker }
    }

    pub fn fetch_task(&self) -> Result<Option<Task>, &'static str> {
        match self.chain.chain_type {
            ChainType::Evm => Ok(self.query_evm_actived_request(&self.chain, &self.worker)?),
            ChainType::Sub => Ok(self.query_sub_actived_request(&self.chain, &self.worker)?),
        }
    }

    fn query_evm_actived_request(
        &self,
        chain: &Chain,
        worker: &AccountInfo,
    ) -> Result<Option<Task>, &'static str> {
        let handler_on_goerli: H160 = H160::from_slice(&chain.handler_contract);
        let transport = Eth::new(PinkHttp::new(&chain.endpoint));
        let handler = Contract::from_json(
            transport,
            handler_on_goerli,
            include_bytes!("./abi/handler.json"),
        )
        .map_err(|_| "ConstructContractFailed")?;

        let worker_address: Address = worker.account20.into();
        pink_extension::debug!(
            "Lookup actived task for worker {:?} on {:?}",
            &hex::encode(worker_address),
            &chain.name
        );

        let request_id: [u8; 32] = resolve_ready(handler.query(
            "getLastActivedRequest",
            worker_address,
            None,
            Options::default(),
            None,
        ))
        .map_err(|_| "FailedGetLastActivedRequest")?;
        if request_id == [0; 32] {
            return Ok(None);
        }
        pink_extension::debug!(
            "getLastActivedRequest, return request_id: {:?}",
            hex::encode(request_id)
        );
        let evm_deposit_data: EvmDepositData = resolve_ready(handler.query(
            "getRequestData",
            request_id,
            None,
            Options::default(),
            None,
        ))
        .map_err(|_| "FailedGetRequestData")?;
        pink_extension::debug!(
            "Fetch deposit data successfully for request {:?} on {:?}, deposit data: {:?}",
            &hex::encode(request_id),
            &chain.name,
            &evm_deposit_data,
        );
        let deposit_data: DepositData = evm_deposit_data.into();
        let task = deposit_data.to_task(&chain.name, request_id)?;
        Ok(Some(task))
    }

    fn query_sub_actived_request(
        &self,
        chain: &Chain,
        worker: &AccountInfo,
    ) -> Result<Option<Task>, &'static str> {
        if let Some(raw_storage) = get_storage(
            &chain.endpoint,
            &storage_map_prefix::<Twox64Concat>(
                &storage_prefix("PalletIndex", "ActivedRequests")[..],
                &worker.account32,
            ),
            None,
        )
        // .log_err("Read storage [actived request] failed")
        .map_err(|_| "FailedGetRequestData")?
        {
            let actived_requests: Vec<[u8; 32]> =
                scale::Decode::decode(&mut raw_storage.as_slice())
                    // .log_err("Decode storage [sub native balance] failed")
                    .map_err(|_| "DecodeStorageFailed")?;
            // println!("actived requests: {:?}", &actived_requests);
            if actived_requests.len() > 0 {
                let oldest_request = actived_requests[0];
                if let Some(raw_storage) = get_storage(
                    &chain.endpoint,
                    &storage_map_prefix::<Twox64Concat>(
                        &storage_prefix("PalletIndex", "DepositRecords")[..],
                        &oldest_request,
                    ),
                    None,
                )
                // .log_err("Read storage [actived request] failed")
                .map_err(|_| "FailedGetRequestData")?
                {
                    let sub_deposit_data: SubDepositData =
                        scale::Decode::decode(&mut raw_storage.as_slice())
                            // .log_err("Decode storage [sub native balance] failed")
                            .map_err(|_| "DecodeStorageFailed")?;
                    pink_extension::debug!(
                        "Fetch deposit data successfully for request {:?} on {:?}, deposit data: {:?}",
                        &hex::encode(oldest_request),
                        &chain.name,
                        &sub_deposit_data,
                    );
                    // println!(
                    //     "Fetch deposit data successfully for request {:?} on {:?}, deposit data: {:?}",
                    //     &hex::encode(oldest_request),
                    //     &chain.name,
                    //     &sub_deposit_data,
                    // );
                    let deposit_data: DepositData = sub_deposit_data.into();
                    let task = deposit_data.to_task(&chain.name, oldest_request)?;
                    // println!("sub task: {:?}", &task);
                    Ok(Some(task))
                } else {
                    Err("DepositInfoNotFound")
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
struct EvmDepositData {
    // TODO: use Bytes
    sender: Address,
    amount: U256,
    recipient: Vec<u8>,
    request: String,
}

impl Detokenize for EvmDepositData {
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
                        deposit_data[2].clone(),
                        deposit_data[3].clone(),
                        deposit_data[4].clone(),
                    ) {
                        (
                            Token::Address(sender),
                            Token::Uint(amount),
                            Token::Bytes(recipient),
                            Token::String(request),
                        ) => Ok(EvmDepositData {
                            sender,
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

// Copy from pallet-index
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct SubDepositData {
    pub sender: [u8; 32],
    pub asset: XcmAssetId,
    pub amount: u128,
    pub recipient: Vec<u8>,
    pub request: Vec<u8>,
}

// Define the structures to parse deposit data json
#[allow(dead_code)]
#[derive(Debug)]
struct DepositData {
    // TODO: use Bytes
    sender: Vec<u8>,
    amount: u128,
    recipient: Vec<u8>,
    request: String,
}

impl From<EvmDepositData> for DepositData {
    fn from(value: EvmDepositData) -> Self {
        Self {
            sender: value.sender.as_bytes().into(),
            amount: value.amount.try_into().expect("Amount overflow"),
            recipient: value.recipient,
            request: value.request,
        }
    }
}

impl From<SubDepositData> for DepositData {
    fn from(value: SubDepositData) -> Self {
        Self {
            sender: value.sender.into(),
            amount: value.amount,
            recipient: value.recipient,
            request: String::from_utf8_lossy(&value.request).into_owned(),
        }
    }
}

impl DepositData {
    fn to_task(&self, source_chain: &str, id: [u8; 32]) -> Result<Task, &'static str> {
        pink_extension::debug!("Trying to parse request data from json string");
        let request_data_json: RequestDataJson =
            pink_json::from_str(&self.request).map_err(|_| "InvalidRequest")?;
        pink_extension::debug!(
            "Parse request data successfully, found {:?} operations",
            request_data_json.len()
        );
        if request_data_json.is_empty() {
            return Err("EmptyTask");
        }
        pink_extension::debug!("Trying to convert request data to task");

        let mut uninitialized_task = Task {
            id,
            source: source_chain.into(),
            sender: self.sender.clone(),
            recipient: self.recipient.clone(),
            ..Default::default()
        };

        // Insert claim step
        uninitialized_task.steps.push(Step {
            meta: StepMeta::Claim(ClaimStep {
                chain: source_chain.into(),
                id,
                asset: self.decode_address(&request_data_json[0].spend_asset)?,
                b0: None,
            }),
            chain: source_chain.into(),
            nonce: None,
        });
        pink_extension::debug!("Insert claim operation in front of existing steps");

        for op in request_data_json.iter() {
            if op.op_type == *"swap" {
                uninitialized_task.steps.push(Step {
                    meta: StepMeta::Swap(SwapStep {
                        spend_asset: self.decode_address(&op.spend_asset)?,
                        receive_asset: self.decode_address(&op.receive_asset)?,
                        chain: op.source_chain.clone(),
                        dex: op.dex.clone(),
                        cap: self.u128_from_string(&op.cap)?,
                        flow: self.u128_from_string(&op.flow)?,
                        impact: self.u128_from_string(&op.impact)?,
                        b0: None,
                        b1: None,
                        spend: self.u128_from_string(&op.spend)?,
                        recipient: None,
                    }),
                    chain: op.source_chain.clone(),
                    nonce: None,
                });
            } else if op.op_type == *"bridge" {
                uninitialized_task.steps.push(Step {
                    meta: StepMeta::Bridge(BridgeStep {
                        from: self.decode_address(&op.spend_asset)?,
                        source_chain: op.source_chain.clone(),
                        to: self.decode_address(&op.receive_asset)?,
                        dest_chain: op.dest_chain.clone(),
                        fee: self.u128_from_string(&op.fee)?,
                        cap: self.u128_from_string(&op.cap)?,
                        flow: self.u128_from_string(&op.flow)?,
                        b0: None,
                        b1: None,
                        amount: self.u128_from_string(&op.spend)?,
                        recipient: None,
                    }),
                    chain: op.source_chain.clone(),
                    nonce: None,
                });
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

    fn decode_address(&self, address: &str) -> Result<Vec<u8>, &'static str> {
        if address.len() < 2 && address.len() % 2 != 0 {
            return Err("InvalidAddress");
        }

        hex::decode(&address[2..]).map_err(|_| "DecodeAddressFailed")
    }
}

type RequestDataJson = Vec<OperationJson>;

#[derive(Deserialize)]
struct OperationJson {
    op_type: String,
    source_chain: String,
    dest_chain: String,
    spend_asset: String,
    receive_asset: String,
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
    fn test_json_parse() {
        let request = "[{\"op_type\":\"swap\",\"source_chain\":\"Moonbeam\",\"dest_chain\":\"Moonbeam\",\"spend_asset\":\"0xAcc15dC74880C9944775448304B263D191c6077F\",\"receive_asset\":\"0xFfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080\",\"dex\":\"BeamSwap\",\"fee\":\"0\",\"cap\":\"0\",\"flow\":\"1000000000000000000\",\"impact\":\"0\",\"spend\":\"1000000000000000000\"},{\"op_type\":\"bridge\",\"source_chain\":\"Moonbeam\",\"dest_chain\":\"Acala\",\"spend_asset\":\"0xFfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080\",\"receive_asset\":\"0x010200411f06080002\",\"dex\":\"null\",\"fee\":\"0\",\"cap\":\"0\",\"flow\":\"700000000\",\"impact\":\"0\",\"spend\":\"700000000\"},{\"op_type\":\"swap\",\"source_chain\":\"Acala\",\"dest_chain\":\"Acala\",\"spend_asset\":\"0x010200411f06080002\",\"receive_asset\":\"0x010200411f06080000\",\"dex\":\"AcalaDex\",\"fee\":\"0\",\"cap\":\"0\",\"flow\":\"700000000\",\"impact\":\"0\",\"spend\":\"700000000\"}]";
        let _request_data_json: RequestDataJson = pink_json::from_str(&request).unwrap();
    }

    #[test]
    // Remove when `handler address is not hardcoded
    #[ignore]
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
                native_asset: vec![0],
                foreign_asset: None,
                handler_contract: hex!("056C0E37d026f9639313C281250cA932C9dbe921").into(),
            },
            worker: AccountInfo {
                account20: worker_address.into(),
                account32: [0; 32],
            },
        }
        .fetch_task()
        .unwrap()
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
            handler_contract: hex!("056C0E37d026f9639313C281250cA932C9dbe921").into(),
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
        assert_eq!(
            claim_step.run(nonce, &context).unwrap(),
            hex::decode("7a0a6ba48285ffb7c0d00e11ad684aa60b30ac6d4b2cce43c6a0fe3f75791caa")
                .unwrap()
        );

        // Wait 60 seconds to let transaction confirmed
        std::thread::sleep(std::time::Duration::from_millis(60000));

        assert_eq!(claim_step.check(nonce, &context).unwrap(), true);
    }

    #[test]
    // #[ignore]
    fn test_fetch_task_from_sub() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        // Worker public key
        let worker_key: [u8; 32] =
            hex!("2eaaf908adda6391e434ff959973019fb374af1076edd4fec55b5e6018b1a955").into();
        let task = ActivedTaskFetcher {
            chain: Chain {
                id: 0,
                name: String::from("Khala"),
                chain_type: ChainType::Sub,
                endpoint: String::from("http://127.0.0.1:30444"),
                native_asset: vec![0],
                foreign_asset: None,
                handler_contract: hex!("00").into(),
            },
            worker: AccountInfo {
                account20: [0; 20],
                account32: worker_key,
            },
        }
        .fetch_task()
        .unwrap()
        .unwrap();
        assert_eq!(task.steps.len(), 3);
        match (
            task.steps[0].meta.clone(),
            task.steps[1].meta.clone(),
            task.steps[2].meta.clone(),
        ) {
            (
                StepMeta::Claim(claim_step),
                StepMeta::Bridge(bridge_meta),
                StepMeta::Swap(swap_meta),
            ) => {
                assert_eq!(claim_step.chain, String::from("Phala"));
                assert_eq!(bridge_meta.amount, 301_000_000_000_000);
                assert_eq!(swap_meta.spend, 1_000_000_000_000_000_000 as u128);
            }
            _ => assert!(false),
        }
    }
}
