use alloc::{string::String, vec::Vec};
use index::graph::{Chain, ChainType, NonceFetcher};
use index::tx;
use scale::{Decode, Encode};

use crate::account::AccountInfo;
use crate::context::Context;
use crate::steps::bridge::BridgeStep;
use crate::steps::swap::SwapStep;
use crate::steps::transfer::TransferStep;
use crate::steps::{Step, StepMeta};
use crate::task::{OnchainTasks, Task, TaskId};
use crate::traits::Runner;
use xcm::latest::AssetId as XcmAssetId;

use pink_subrpc::{
    create_transaction, get_storage,
    hasher::Twox64Concat,
    send_transaction,
    storage::{storage_map_prefix, storage_prefix},
    ExtraParam,
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
use pink_extension::ResultExt;
use serde::Deserialize;

use super::ExtraResult;

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
}

impl Runner for ClaimStep {
    fn runnable(
        &self,
        nonce: u64,
        context: &Context,
        client: Option<&mut SubstrateRollupClient>,
    ) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);
        let chain = &context
            .graph
            .get_chain(self.chain.clone())
            .ok_or("MissingChain")?;
        let account = match chain.chain_type {
            index::graph::ChainType::Evm => worker_account.account20.to_vec(),
            index::graph::ChainType::Sub => worker_account.account32.to_vec(),
        };
        // if ok then not runnable
        if tx::is_tx_ok(&chain.tx_indexer, &account, nonce).or(Err("Indexer failure"))? {
            Ok(false)
        } else {
            // check if task exists in rollup storage
            // if so, claim it, and regard it as runnable: Ok(true)
            // if no task exists in rollup storage, then this is step is not runnable
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
            ChainType::Sub => Ok(self.claim_sub_actived_tasks(chain, self.id, &signer, nonce)?),
        }
    }

    fn check(&self, nonce: u64, context: &Context) -> Result<(bool, ExtraResult), &'static str> {
        let worker_account = AccountInfo::from(context.signer);
        let chain = context
            .graph
            .get_chain(self.chain.clone())
            .ok_or("MissingChain")?;
        let account = match chain.chain_type {
            index::graph::ChainType::Evm => worker_account.account20.to_vec(),
            index::graph::ChainType::Sub => worker_account.account32.to_vec(),
        };

        Ok((
            tx::is_tx_ok(&chain.tx_indexer, &account, nonce).or(Err("Indexer failure"))?,
            ExtraResult::None,
        ))
    }

    fn sync_check(
        &self,
        nonce: u64,
        context: &Context,
    ) -> Result<(bool, ExtraResult), &'static str> {
        pink_extension::debug!("Claim step sync checking: {:?}", self);
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
            include_bytes!("../abi/handler.json"),
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
            "Submit transaction to claim task {:?} on {:?}, tx id: {:?}",
            hex::encode(task_id),
            &chain.name,
            hex::encode(tx_id.clone().as_bytes())
        );
        Ok(tx_id.as_bytes().to_vec())
    }

    fn claim_sub_actived_tasks(
        &self,
        chain: Chain,
        task_id: TaskId,
        worker_key: &[u8; 32],
        nonce: u64,
    ) -> Result<Vec<u8>, &'static str> {
        let signed_tx = create_transaction(
            worker_key,
            "phala",
            &chain.endpoint,
            // Pallet id of `pallet-index`
            *chain
                .handler_contract
                .first()
                .ok_or("ClaimMissingPalletId")?,
            // Call index of `claim_task`
            0x03u8,
            task_id,
            ExtraParam {
                tip: 0,
                nonce: Some(nonce),
                era: None,
            },
        )
        .map_err(|_| "ClaimInvalidSignature")?;
        let tx_id =
            send_transaction(&chain.endpoint, &signed_tx).map_err(|_| "ClaimSubmitFailed")?;
        pink_extension::info!(
            "Submit transaction to claim task {:?} on {:?}, tx id: {:?}",
            hex::encode(task_id),
            &chain.name,
            hex::encode(tx_id.clone())
        );
        Ok(tx_id)
    }
}

/// Fetch actived tasks from blockchains and construct a `Task` from it.
/// If the given chain is EVM based, fetch tasks from solidity-based smart contract storage through RPC task.
/// For example, call RPC methods defined here:
///     https://github.com/Phala-Network/index-solidity/blob/7b4458f9b8277df8a1c705a4d0f264476f42fcf2/contracts/Handler.sol#L147
///     https://github.com/Phala-Network/index-solidity/blob/7b4458f9b8277df8a1c705a4d0f264476f42fcf2/contracts/Handler.sol#L165
/// If the given chain is Substrate based, fetch tasks from pallet storage through RPC task.
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
            ChainType::Evm => Ok(self.query_evm_actived_task(&self.chain, &self.worker)?),
            ChainType::Sub => Ok(self.query_sub_actived_task(&self.chain, &self.worker)?),
        }
    }

    fn query_evm_actived_task(
        &self,
        chain: &Chain,
        worker: &AccountInfo,
    ) -> Result<Option<Task>, &'static str> {
        let handler: H160 = H160::from_slice(&chain.handler_contract);
        let transport = Eth::new(PinkHttp::new(&chain.endpoint));
        let handler =
            Contract::from_json(transport, handler, include_bytes!("../abi/handler.json"))
                .map_err(|_| "ConstructContractFailed")?;

        let worker_address: Address = worker.account20.into();
        pink_extension::debug!(
            "Lookup actived task for worker {:?} on {:?}",
            &hex::encode(worker_address),
            &chain.name
        );

        let task_id: [u8; 32] = resolve_ready(handler.query(
            "getLastActivedTask",
            worker_address,
            None,
            Options::default(),
            None,
        ))
        .map_err(|_| "FailedGetLastActivedTask")?;
        if task_id == [0; 32] {
            return Ok(None);
        }
        pink_extension::debug!(
            "getLastActivedTask, return task_id: {:?}",
            hex::encode(task_id)
        );
        let evm_deposit_data: EvmDepositData =
            resolve_ready(handler.query("getTaskData", task_id, None, Options::default(), None))
                .map_err(|_| "FailedGetTaskData")?;
        pink_extension::debug!(
            "Fetch deposit data successfully for task {:?} on {:?}, deposit data: {:?}",
            &hex::encode(task_id),
            &chain.name,
            &evm_deposit_data,
        );
        let deposit_data: DepositData = evm_deposit_data.into();
        let task = deposit_data.to_task(&chain.name, task_id, self.worker.account32)?;

        pink_extension::debug!(
            "ActivedTaskFetcher::query_evm_actived_task safely exits with {:?}",
            task
        );
        Ok(Some(task))
    }

    fn query_sub_actived_task(
        &self,
        chain: &Chain,
        worker: &AccountInfo,
    ) -> Result<Option<Task>, &'static str> {
        if let Some(raw_storage) = get_storage(
            &chain.endpoint,
            &storage_map_prefix::<Twox64Concat>(
                &storage_prefix("PalletIndex", "ActivedTasks")[..],
                &worker.account32,
            ),
            None,
        )
        .log_err("Read storage [actived task] failed")
        .map_err(|_| "FailedGetTaskData")?
        {
            let actived_tasks: Vec<[u8; 32]> = scale::Decode::decode(&mut raw_storage.as_slice())
                .log_err("Decode storage [actived task] failed")
                .map_err(|_| "DecodeStorageFailed")?;
            if !actived_tasks.is_empty() {
                let oldest_task = actived_tasks[0];
                if let Some(raw_storage) = get_storage(
                    &chain.endpoint,
                    &storage_map_prefix::<Twox64Concat>(
                        &storage_prefix("PalletIndex", "DepositRecords")[..],
                        &oldest_task,
                    ),
                    None,
                )
                .log_err("Read storage [actived task] failed")
                .map_err(|_| "FailedGetDepositData")?
                {
                    let sub_deposit_data: SubDepositData =
                        scale::Decode::decode(&mut raw_storage.as_slice())
                            .log_err("Decode storage [deposit data] failed")
                            .map_err(|_| "DecodeStorageFailed")?;
                    pink_extension::debug!(
                        "Fetch deposit data successfully for task {:?} on {:?}, deposit data: {:?}",
                        &hex::encode(oldest_task),
                        &chain.name,
                        &sub_deposit_data,
                    );
                    let deposit_data: DepositData = sub_deposit_data.into();
                    let task =
                        deposit_data.to_task(&chain.name, oldest_task, self.worker.account32)?;
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
    task: String,
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
                            Token::String(task),
                        ) => Ok(EvmDepositData {
                            sender,
                            amount,
                            recipient,
                            task,
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
    pub task: Vec<u8>,
}

// Define the structures to parse deposit data json
#[allow(dead_code)]
#[derive(Debug)]
struct DepositData {
    // TODO: use Bytes
    sender: Vec<u8>,
    amount: u128,
    recipient: Vec<u8>,
    task: String,
}

impl From<EvmDepositData> for DepositData {
    fn from(value: EvmDepositData) -> Self {
        Self {
            sender: value.sender.as_bytes().into(),
            amount: value.amount.try_into().expect("Amount overflow"),
            recipient: value.recipient,
            task: value.task,
        }
    }
}

impl From<SubDepositData> for DepositData {
    fn from(value: SubDepositData) -> Self {
        Self {
            sender: value.sender.into(),
            amount: value.amount,
            recipient: value.recipient,
            task: String::from_utf8_lossy(&value.task).into_owned(),
        }
    }
}

impl DepositData {
    fn to_task(
        &self,
        source_chain: &str,
        id: [u8; 32],
        worker: [u8; 32],
    ) -> Result<Task, &'static str> {
        pink_extension::debug!("Trying to parse task data from json string");
        let task_data_json: TaskDataJson =
            pink_json::from_str(&self.task).map_err(|_| "InvalidTask")?;
        pink_extension::debug!(
            "Parse task data successfully, found {:?} operations",
            task_data_json.len()
        );
        if task_data_json.is_empty() {
            return Err("EmptyTask");
        }
        pink_extension::debug!("Trying to convert task data to task");

        let mut uninitialized_task = Task {
            id,
            source: source_chain.into(),
            sender: self.sender.clone(),
            recipient: self.recipient.clone(),
            worker,
            ..Default::default()
        };

        // Insert claim step
        uninitialized_task.steps.push(Step {
            meta: StepMeta::Claim(ClaimStep {
                chain: source_chain.into(),
                id,
                asset: self.decode_address(&task_data_json[0].spend_asset)?,
            }),
            chain: source_chain.into(),
            nonce: None,
        });
        pink_extension::debug!("Insert claim operation in front of existing steps");

        for op in task_data_json.iter() {
            if op.op_type == *"swap" {
                uninitialized_task.steps.push(Step {
                    meta: StepMeta::Swap(SwapStep {
                        spend_asset: self.decode_address(&op.spend_asset)?,
                        receive_asset: self.decode_address(&op.receive_asset)?,
                        chain: op.source_chain.clone(),
                        name: op.tag.clone(),
                        spend: self.u128_from_string(&op.spend)?,
                        receive_max: self.u128_from_string(&op.receive_max)?,
                        receive_min: self.u128_from_string(&op.receive_min)?,
                        recipient: Default::default(),
                    }),
                    chain: op.source_chain.clone(),
                    nonce: None,
                });
            } else if op.op_type == *"bridge" {
                uninitialized_task.steps.push(Step {
                    meta: StepMeta::Bridge(BridgeStep {
                        name: op.tag.clone(),
                        from: self.decode_address(&op.spend_asset)?,
                        source_chain: op.source_chain.clone(),
                        to: self.decode_address(&op.receive_asset)?,
                        dest_chain: op.dest_chain.clone(),
                        spend: self.u128_from_string(&op.spend)?,
                        receive_max: self.u128_from_string(&op.receive_max)?,
                        receive_min: self.u128_from_string(&op.receive_min)?,
                        recipient: Default::default(),
                        block_number: 0,
                        index_in_block: 0,
                    }),
                    chain: op.source_chain.clone(),
                    nonce: None,
                });
            } else if op.op_type == *"transfer" {
                uninitialized_task.steps.push(Step {
                    meta: StepMeta::Transfer(TransferStep {
                        asset: self.decode_address(&op.spend_asset)?,
                        amount: self.u128_from_string(&op.spend)?,
                        chain: op.source_chain.clone(),
                        spend: self.u128_from_string(&op.spend)?,
                        receive_max: self.u128_from_string(&op.receive_max)?,
                        receive_min: self.u128_from_string(&op.receive_min)?,
                        recipient: Default::default(),
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

    fn decode_address(&self, address: &str) -> Result<Vec<u8>, &'static str> {
        if address.len() < 2 && address.len() % 2 != 0 {
            return Err("InvalidAddress");
        }

        hex::decode(&address[2..]).map_err(|_| "DecodeAddressFailed")
    }
}

type TaskDataJson = Vec<OperationJson>;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OperationJson {
    op_type: String,
    tag: String,
    source_chain: String,
    dest_chain: String,
    spend_asset: String,
    receive_asset: String,
    spend: String,
    receive_min: String,
    receive_max: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use alloc::vec;
    use dotenv::dotenv;
    use hex_literal::hex;
    use index::{
        graph::{BalanceFetcher, Chain, ChainType, Graph},
        utils::ToArray,
    };

    #[test]
    fn test_json_parse() {
        let request = r#"
        [
            {
              "tag": "AcalaDex",
              "spend": "9577071932307798",
              "sourceChain": "Acala",
              "destChain": "Acala",
              "spendAsset": "ACA",
              "receiveAsset": "AUSD",
              "receiveMin": "1970478348022065",
              "receiveMax": "2092363606662605",
              "opType": "dex"
            },
            {
              "tag": "Acala",
              "spend": "2031420977342335",
              "sourceChain": "Acala",
              "destChain": "Acala",
              "spendAsset": "AUSD",
              "receiveAsset": "AUSD",
              "receiveMin": "1970275743439996",
              "receiveMax": "2092148469838346",
              "opType": "transfer"
            }
        ]
        "#;
        let _request_data_json: TaskDataJson = pink_json::from_str(&request).unwrap();
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
                tx_indexer: Default::default(),
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
                assert_eq!(bridge_meta.spend, 12_000_000);
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
            tx_indexer: Default::default(),
        };

        let claim_step = ClaimStep {
            chain: String::from("Goerli"),
            id: hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap()
                .to_array(),
            asset: hex::decode("B376b0Ee6d8202721838e76376e81eEc0e2FE864").unwrap(),
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
            },
            worker_accounts: vec![],
            bridge_executors: vec![],
            dex_executors: vec![],
            transfer_executors: vec![],
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

        assert_eq!(claim_step.check(nonce, &context).unwrap().0, true);
    }

    #[test]
    #[ignore]
    fn test_fetch_task_from_sub() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        // Worker public key
        let worker_key: [u8; 32] =
            hex!("2eaaf908adda6391e434ff959973019fb374af1076edd4fec55b5e6018b1a955").into();
        // We already deposit task with scritps/sub-depopsit.js
        let task = ActivedTaskFetcher {
            chain: Chain {
                id: 0,
                name: String::from("Khala"),
                chain_type: ChainType::Sub,
                endpoint: String::from("http://127.0.0.1:30444"),
                native_asset: vec![0],
                foreign_asset: None,
                handler_contract: hex!("00").into(),
                tx_indexer: Default::default(),
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
                assert_eq!(claim_step.chain, String::from("Khala"));
                assert_eq!(bridge_meta.spend, 301_000_000_000_000);
                assert_eq!(swap_meta.spend, 1_000_000_000_000_000_000 as u128);
            }
            _ => assert!(false),
        }
    }

    #[test]
    #[ignore]
    fn test_claim_task_from_sub_chain() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        // This key is just for test, never put real money in it.
        let mock_worker_prv_key: [u8; 32] =
            hex!("3a531c56b5441c165d2975d186d0c816c4e181da33e89e6ae751fceb77ea970b").into();
        let mock_worker_pub_key: [u8; 32] =
            hex!("2eaaf908adda6391e434ff959973019fb374af1076edd4fec55b5e6018b1a955").into();
        // Current transaction count of the mock worker account
        let nonce = 0;
        // Encoded MultiLocation::here()
        let pha: Vec<u8> = hex!("010100cd1f").into();
        let khala = Chain {
            id: 0,
            name: String::from("Khala"),
            chain_type: ChainType::Sub,
            endpoint: String::from("http://127.0.0.1:30444"),
            native_asset: pha.clone(),
            foreign_asset: None,
            handler_contract: hex!("79").into(),
            tx_indexer: Default::default(),
        };

        let claim_step = ClaimStep {
            chain: String::from("Khala"),
            id: hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap()
                .to_array(),
            asset: pha.clone(),
        };
        let context = Context {
            signer: mock_worker_prv_key,
            graph: Graph {
                chains: vec![khala.clone()],
                ..Default::default()
            },
            worker_accounts: vec![],
            bridge_executors: vec![],
            dex_executors: vec![],
            transfer_executors: vec![],
        };
        // Send claim transaction, we already deposit task with scritps/sub-depopsit.js
        assert_eq!(claim_step.run(nonce, &context).is_ok(), true);

        // Wait 30 seconds to let transaction confirmed
        std::thread::sleep(std::time::Duration::from_millis(30000));

        assert_eq!(claim_step.check(nonce, &context).unwrap().0, true);
        // After claim, asset sent from pallet-index account to worker account
        assert_eq!(
            khala.get_balance(pha, mock_worker_pub_key.into()).unwrap() - 301_000_000_000_000u128
                > 0,
            true
        );
    }
}
