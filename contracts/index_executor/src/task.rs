use super::account::AccountInfo;
use super::context::Context;
use super::traits::Runner;
use crate::chain::{Chain, ChainType, NonceFetcher};
use crate::price;
use crate::step::{MultiStep, Simulate as StepSimulate};
use crate::storage::StorageClient;
use crate::tx;

use alloc::{string::String, vec, vec::Vec};
use ink::storage::Mapping;
use scale::{Decode, Encode};

use pink_subrpc::{create_transaction, send_transaction, ExtraParam};

use pink_web3::{
    api::{Eth, Namespace},
    contract::{Contract, Options},
    keys::pink::KeyPair,
    signing::Key,
    transports::{resolve_ready, PinkHttp},
    types::{H160, U256},
};

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum TaskStatus {
    /// Task initial confirmed by user on source chain.
    Actived,
    /// Task has been initialized, e.g. being applied nonce.
    Initialized,
    /// Indicating that task already been claimed on source chain along with the transaction
    Claimed(Vec<u8>),
    /// Task is being executing with step index.
    /// Transaction can be indentified by worker account nonce on specific chain
    /// [step_index, worker_nonce]
    Executing(u8, Option<u64>),
    /// Last step of task has been executed successfully on dest chain.
    Completed,
}

pub type TaskId = [u8; 32];

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Task {
    // Task id
    pub id: TaskId,
    // Allocated worker account public key to execute the task
    pub worker: [u8; 32],
    // Task status
    pub status: TaskStatus,
    // Source chain name
    pub source: String,
    // Amount of first spend asset
    pub amount: u128,
    // Fee represented by spend asset calculated when claim
    pub fee: Option<u128>,
    // Nonce applied to claim task froom source chain
    pub claim_nonce: Option<u64>,
    // Transaction hash of claim operation
    pub claim_tx: Option<Vec<u8>>,
    /// Steps  after merged, those actually will be executed
    pub merged_steps: Vec<MultiStep>,
    /// Transaction hash of each step operation
    pub execute_txs: Vec<Vec<u8>>,
    /// Current step index that is executing
    pub execute_index: u8,
    /// Sender address on source chain
    pub sender: Vec<u8>,
    /// Recipient address on dest chain
    pub recipient: Vec<u8>,
    // Retry counter, retry counter will be cleared after one step executed successfully
    pub retry_counter: u8,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: [0; 32],
            worker: [0; 32],
            status: TaskStatus::Actived,
            source: String::default(),
            amount: 0,
            fee: None,
            claim_nonce: None,
            claim_tx: None,
            merged_steps: vec![],
            execute_txs: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: vec![],
            retry_counter: 0,
        }
    }
}

impl sp_std::fmt::Debug for Task {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
        f.debug_struct("Task")
            .field("id", &hex::encode(self.id))
            .field("worker", &hex::encode(self.worker))
            .field("status", &self.status)
            .field("source", &self.source)
            .field("amount", &self.amount)
            .field("claim_nonce", &self.claim_nonce)
            .field("merged_steps", &self.merged_steps)
            .field("execute_index", &self.execute_index)
            .field("sender", &hex::encode(&self.sender))
            .field("recipient", &hex::encode(&self.recipient))
            .field("retry_counter", &self.retry_counter)
            .finish()
    }
}

impl Task {
    // Initialize task
    pub fn init(&mut self, context: &Context, client: &StorageClient) -> Result<(), &'static str> {
        if let Some((task, _)) = client
            .read::<Task>(&self.id)
            .map_err(|_| "FailedToReadStorage")?
        {
            pink_extension::debug!(
                "Task {:?} already initialized, will check if it is missed to be added to execute",
                hex::encode(self.id)
            );
            if client
                .read::<TaskId>(&self.worker)
                .map_err(|_| "FailedToReadStorage")?
                .is_none()
                // Still need to check status in case task actually has completed
                && task.status == TaskStatus::Initialized
            {
                client.insert(&self.worker, &self.id.encode())?;
            }
        } else {
            // Apply worker nonce for each step in task
            self.apply_nonce(0, context, client)?;

            // TODO: query initial balance of worker account and setup to specific step
            self.status = TaskStatus::Initialized;
            self.execute_index = 0;

            client.insert(self.id.as_ref(), &self.encode())?;
            client.insert(&self.worker, &self.id.encode())?;
        }

        Ok(())
    }

    pub fn execute(
        &mut self,
        context: &Context,
        client: &StorageClient,
    ) -> Result<TaskStatus, &'static str> {
        // Check claim before executing
        if !(self.has_claimed(context))? {
            pink_extension::debug!(
                "Task {:?} already exist in storage, but hasn't been claimed, try claim it with worker {:?} and return.",
                hex::encode(self.id),
                hex::encode(self.worker),
            );
            let claim_tx = self.claim(context)?;
            self.claim_tx = Some(claim_tx);
            return Ok(self.status.clone());
        }

        let step_count = self.merged_steps.len();
        match self.merged_steps[self.execute_index as usize].can_run(
            // An executing task must have nonce applied
            self.merged_steps[self.execute_index as usize]
                .get_nonce()
                .unwrap(),
            context,
            None,
        ) {
            // If step already executed successfully, execute next step
            Ok(true) => {
                // If all step executed successfully, set task as `Completed`
                if self.execute_index as usize == (step_count - 1) {
                    self.status = TaskStatus::Completed;
                    return Ok(self.status.clone());
                }

                self.execute_index += 1;
                self.retry_counter = 0;

                // Settle last step before execute next step
                let settle_balance =
                    self.merged_steps[(self.execute_index - 1) as usize].settle(context)?;
                pink_extension::debug!(
                    "Finished previous step execution, settle balance of last step[{:?}], settle amount: {:?}",
                    (self.execute_index - 1),
                    settle_balance,
                );

                pink_extension::debug!("Update sepnd amount of next executing step");
                self.merged_steps[self.execute_index as usize].set_spend(settle_balance);

                // FIXME: handle returned error
                let _ = self.execute_step(context, client)?;
            }
            // There are several situations that indexer return `false`:
            // - Step hasn't been executed yet
            // - Step failed to execute
            // - Step has been executed, but off-chain indexer hasn't caught up
            Ok(false) => {
                pink_extension::debug!(
                    "Current step has not been executed or failed to execute, retry step {:?}",
                    (self.execute_index),
                );
                // Since we don't actually understand what happened, retry is the only choice.
                // To avoid we retry too many times, we involved `retry_counter`
                if self.retry_counter < 10 {
                    self.retry_counter += 1;
                    // FIXME: handle returned error
                    let _ = self.execute_step(context, client)?;
                } else {
                    return Err("TooManyRetry");
                }
            }
            Err(e) => return Err(e),
        }

        Ok(self.status.clone())
    }

    /// Check and execute a single step. Only can be executed when the step is ready to run.
    ///
    /// Note this method assume that the last step has been settled, e.g. finished
    pub fn execute_step(
        &mut self,
        context: &Context,
        client: &StorageClient,
    ) -> Result<TaskStatus, &'static str> {
        // An executing task must have nonce applied
        let nonce = self.merged_steps[self.execute_index as usize]
            .get_nonce()
            .unwrap();

        if self.merged_steps[self.execute_index as usize].can_run(nonce, context, Some(client))
            == Ok(true)
        {
            pink_extension::debug!(
                "Trying to run step[{:?}] with nonce {:?}",
                self.execute_index,
                nonce
            );
            self.status = TaskStatus::Executing(self.execute_index, Some(nonce));
            let execute_tx = self.merged_steps[self.execute_index as usize].run(nonce, context)?;
            if self.execute_txs.len() == self.execute_index as usize + 1 {
                // Not the first time to execute the step, just replace it with new tx hash
                self.execute_txs[self.execute_index as usize] = execute_tx;
            } else {
                self.execute_txs.push(execute_tx);
            }
        } else {
            pink_extension::debug!("Step[{:?}] not runnable, return", self.execute_index);
        }
        Ok(self.status.clone())
    }

    /// Delete task record from on-chain storage
    pub fn destroy(&mut self, client: &StorageClient) -> Result<(), &'static str> {
        let (_, running_task_doc) = client
            .read::<TaskId>(&self.worker)?
            .ok_or("TaskNotBeingExecuted")?;
        client.delete(&self.worker, running_task_doc)?;

        Ok(())
    }

    pub fn reapply_nonce(
        &mut self,
        start_index: u64,
        context: &Context,
        client: &StorageClient,
    ) -> Result<(), &'static str> {
        self.apply_nonce(start_index, context, client)
    }

    fn apply_nonce(
        &mut self,
        start_index: u64,
        context: &Context,
        _client: &StorageClient,
    ) -> Result<(), &'static str> {
        let mut nonce_map: Mapping<String, u64> = Mapping::default();

        // Apply claim nonce if hasn't claimed
        if self.claim_nonce.is_none() || !self.has_claimed(context)? {
            let claim_nonce = self.get_nonce(context, &self.source)?;
            nonce_map.insert(self.source.clone(), &(claim_nonce + 1));
            self.claim_nonce = Some(claim_nonce);
        }

        // Apply nonce for each step
        for index in start_index as usize..self.merged_steps.len() {
            let nonce: u64 =
                match nonce_map.get(&self.merged_steps[index].as_single_step().source_chain) {
                    Some(nonce) => nonce,
                    None => self.get_nonce(
                        context,
                        &self.merged_steps[index].as_single_step().source_chain,
                    )?,
                };
            self.merged_steps[index].set_nonce(nonce);
            // Increase nonce by 1
            nonce_map.insert(
                self.merged_steps[index]
                    .as_single_step()
                    .source_chain
                    .clone(),
                &(nonce + 1),
            );
        }

        Ok(())
    }

    fn get_nonce(&self, context: &Context, chain: &String) -> Result<u64, &'static str> {
        let chain: Chain = context.registry.get_chain(chain).ok_or("MissingChain")?;
        let account_info = context.get_account(self.worker).ok_or("WorkerNotFound")?;
        let account = match chain.chain_type {
            ChainType::Evm => account_info.account20.to_vec(),
            ChainType::Sub => account_info.account32.to_vec(),
            // ChainType::Unknown => panic!("chain not supported!"),
        };
        let nonce = chain.get_nonce(account).map_err(|_| "FetchNonceFailed")?;
        Ok(nonce)
    }

    fn claim(&mut self, context: &Context) -> Result<Vec<u8>, &'static str> {
        let chain = context
            .registry
            .get_chain(&self.source)
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;
        let claim_nonce = self.claim_nonce.ok_or("MissingClaimNonce")?;

        match chain.chain_type {
            ChainType::Evm => {
                Ok(self.claim_evm_actived_tasks(chain, self.id, context, claim_nonce)?)
            }
            ChainType::Sub => {
                Ok(self.claim_sub_actived_tasks(chain, self.id, context, claim_nonce)?)
            }
        }
    }

    fn has_claimed(&self, context: &Context) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);
        let chain = context
            .registry
            .get_chain(&self.source)
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;
        let account = match chain.chain_type {
            ChainType::Evm => worker_account.account20.to_vec(),
            ChainType::Sub => worker_account.account32.to_vec(),
        };
        let claim_nonce = self.claim_nonce.ok_or("MissingClaimNonce")?;

        // Check if already claimed success
        let onchain_nonce = worker_account.get_nonce(&self.source, context)?;
        if onchain_nonce > claim_nonce {
            if tx::has_confirmed(&chain.tx_indexer_url, &account, claim_nonce)? {
                Ok(true)
            } else {
                Err("ClaimFailed")
            }
        } else {
            Ok(false)
        }
    }

    fn claim_evm_actived_tasks(
        &mut self,
        chain: Chain,
        task_id: TaskId,
        context: &Context,
        nonce: u64,
    ) -> Result<Vec<u8>, &'static str> {
        let handler: H160 = H160::from_slice(&chain.handler_contract);
        let transport = Eth::new(PinkHttp::new(chain.endpoint));
        let handler = Contract::from_json(transport, handler, include_bytes!("./abi/handler.json"))
            .map_err(|_| "ConstructContractFailed")?;
        let worker = KeyPair::from(context.signer);

        let fee = self.calculate_fee(context)?;
        if fee >= self.amount {
            return Err("TooExpensive");
        }

        self.fee = Some(fee);
        // We call claimAndBatchCall so that first step will be executed along with the claim operation
        let first_step = &mut self.merged_steps[0];
        // FIXME: We definitely need to consider the minimal amount allowed
        first_step.set_spend(self.amount - fee);
        first_step.sync_origin_balance(context)?;
        let calls = first_step.derive_calls(context)?;
        pink_extension::debug!("Calls will be executed along with claim: {:?}", &calls);

        let params = (task_id, U256::from(fee), calls);
        // Estiamte gas before submission
        let gas = resolve_ready(handler.estimate_gas(
            "claimAndBatchCall",
            params.clone(),
            worker.address(),
            Options::default(),
        ))
        .map_err(|e| {
            pink_extension::error!(
                "claimAndBatchCall: failed to estimate gas cost with error {:?}",
                &e
            );
            "GasEstimateFailed"
        })?;

        // Submit the claim transaction
        let tx_id = resolve_ready(handler.signed_call(
            "claimAndBatchCall",
            params,
            Options::with(|opt| {
                // Give 50% gas for potentially gas exceeding
                opt.gas = Some(gas * U256::from(15) / U256::from(10));
                opt.nonce = Some(nonce.into());
            }),
            worker,
        ))
        .map_err(|e| {
            pink_extension::error!("claimAndBatchCall: failed to submit tx with error {:?}", &e);
            "ClaimSubmitFailed"
        })?
        .as_bytes()
        .to_vec();

        // Merge nonce to let check for first step work properly
        first_step.set_nonce(self.claim_nonce.unwrap());
        // Set first step execution transaction hash
        if self.execute_txs.is_empty() {
            self.execute_txs.push(tx_id.clone());
        } else {
            self.execute_txs[0] = tx_id.clone();
        }

        pink_extension::info!(
            "Submit transaction to claim task {:?} on {:?}, tx id: {:?}",
            hex::encode(task_id),
            &chain.name,
            hex::encode(&tx_id)
        );
        Ok(tx_id)
    }

    fn claim_sub_actived_tasks(
        &mut self,
        chain: Chain,
        task_id: TaskId,
        context: &Context,
        nonce: u64,
    ) -> Result<Vec<u8>, &'static str> {
        let signed_tx = create_transaction(
            &context.signer,
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
            "Submit transaction to claim task {:?} on ${:?}, tx id: {:?}",
            hex::encode(task_id),
            &chain.name,
            hex::encode(tx_id.clone())
        );
        let first_step = &mut self.merged_steps[0];
        first_step.set_spend(self.amount);
        Ok(tx_id)
    }

    fn calculate_fee(&self, context: &Context) -> Result<u128, &'static str> {
        let mut fee_in_usd: u32 = 0;
        for step in self.merged_steps.iter() {
            let mut simulate_step = step.clone(); // A minimal amount
            simulate_step.set_spend(1_000_000_000);
            let step_simulate_result = simulate_step
                .simulate(context)
                .map_err(|_| "SimulateRrror")?;

            // We only need to collect tx fee and extra protocol fee, those fee are actually paied by worker
            // during execution
            fee_in_usd += step_simulate_result.tx_fee_in_usd
                + step_simulate_result
                    .action_extra_info
                    .extra_proto_fee_in_usd;
        }

        let asset_location = self.merged_steps[0].as_single_step().spend_asset;
        let asset_info = context
            .registry
            .get_asset(&self.source, &asset_location)
            .ok_or("MissingAssetInfo")?;
        let asset_price =
            price::get_price(&self.source, &asset_location).ok_or("MissingPriceData")?;
        Ok(10u128.pow(asset_info.decimals as u32) * fee_in_usd as u128
            / asset_price as u128
            / 10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountInfo;
    use crate::chain::{BalanceFetcher, Chain, ChainType};
    use crate::registry::Registry;
    use crate::step::StepInput;
    use crate::task_fetcher::ActivedTaskFetcher;
    use crate::utils::ToArray;
    use dotenv::dotenv;
    use hex_literal::hex;
    use primitive_types::H160;

    #[test]
    // Remove when `handler address is not hardcoded
    #[ignore]
    fn test_fetch_task_from_evm() {
        dotenv().ok();

        pink_extension_runtime::mock_ext::mock_all_ext();

        let client: StorageClient = StorageClient::new("url".to_string(), "key".to_string());
        let worker_address: H160 = hex!("f60dB2d02af3f650798b59CB6D453b78f2C1BC90").into();
        let _task = ActivedTaskFetcher {
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
                tx_indexer_url: Default::default(),
            },
            worker: AccountInfo {
                account20: worker_address.into(),
                account32: [0; 32],
            },
        }
        .fetch_task(&client)
        .unwrap()
        .unwrap();
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
            tx_indexer_url: Default::default(),
        };

        let context = Context {
            signer: mock_worker_key,
            registry: &Registry {
                chains: vec![goerli],
                assets: vec![],
            },
            worker_accounts: vec![],
        };
        let mut task = Task::default();
        task.id = hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
            .unwrap()
            .to_array();
        task.claim_nonce = Some(nonce);

        // Send claim transaction
        // https://goerli.etherscan.io/tx/0x7a0a6ba48285ffb7c0d00e11ad684aa60b30ac6d4b2cce43c6a0fe3f75791caa
        assert_eq!(
            task.claim(&context).unwrap(),
            hex::decode("7a0a6ba48285ffb7c0d00e11ad684aa60b30ac6d4b2cce43c6a0fe3f75791caa")
                .unwrap()
        );

        // Wait 60 seconds to let transaction confirmed
        std::thread::sleep(std::time::Duration::from_millis(60000));

        assert_eq!(task.has_claimed(&context).unwrap(), true);
    }

    #[test]
    #[ignore]
    fn test_fetch_task_from_sub() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let client: StorageClient = StorageClient::new("url".to_string(), "key".to_string());
        // Worker public key
        let worker_key: [u8; 32] =
            hex!("2eaaf908adda6391e434ff959973019fb374af1076edd4fec55b5e6018b1a955").into();
        // We already deposit task with scritps/sub-depopsit.js
        let _task = ActivedTaskFetcher {
            chain: Chain {
                id: 0,
                name: String::from("Khala"),
                chain_type: ChainType::Sub,
                endpoint: String::from("http://127.0.0.1:30444"),
                native_asset: vec![0],
                foreign_asset: None,
                handler_contract: hex!("00").into(),
                tx_indexer_url: Default::default(),
            },
            worker: AccountInfo {
                account20: [0; 20],
                account32: worker_key,
            },
        }
        .fetch_task(&client)
        .unwrap()
        .unwrap();
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
            tx_indexer_url: Default::default(),
        };

        let context = Context {
            signer: mock_worker_prv_key,
            registry: &Registry {
                chains: vec![khala.clone()],
                assets: vec![],
            },
            worker_accounts: vec![],
        };
        let mut task = Task::default();
        task.id = hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
            .unwrap()
            .to_array();
        task.claim_nonce = Some(nonce);

        // Send claim transaction, we already deposit task with scritps/sub-depopsit.js
        assert_eq!(
            task.claim(&context).unwrap(),
            hex::decode("7a0a6ba48285ffb7c0d00e11ad684aa60b30ac6d4b2cce43c6a0fe3f75791caa")
                .unwrap()
        );

        // Wait 60 seconds to let transaction confirmed
        std::thread::sleep(std::time::Duration::from_millis(60000));

        assert_eq!(task.has_claimed(&context).unwrap(), true);

        // After claim, asset sent from pallet-index account to worker account
        assert_eq!(
            khala.get_balance(pha, mock_worker_pub_key.into()).unwrap() - 301_000_000_000_000u128
                > 0,
            true
        );
    }

    #[ink::test]
    fn test_get_evm_account_nonce() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();
        let _ = env_logger::try_init();

        let goerli = Chain {
            id: 1,
            name: String::from("Goerli"),
            endpoint: String::from(
                "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
            ),
            chain_type: ChainType::Evm,
            native_asset: vec![0],
            foreign_asset: None,
            handler_contract: "0x056C0E37d026f9639313C281250cA932C9dbe921".into(),
            tx_indexer_url: Default::default(),
        };
        assert_eq!(
            goerli
                .get_nonce(hex!("0E275F8839b788B2674935AD97C01cF73A9E8c41").into())
                .unwrap(),
            2
        );
    }

    #[ignore]
    #[ink::test]
    fn task_init_should_work() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();
        // Secret key of test account `//Alice`
        let _sk_alice = hex!("e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a");

        // Prepare worker accounts
        let mut worker_accounts: Vec<AccountInfo> = vec![];
        for index in 0..10 {
            let private_key = pink_web3::keys::pink::KeyPair::derive_keypair(
                &[b"worker".to_vec(), [index].to_vec()].concat(),
            )
            .private_key();
            worker_accounts.push(AccountInfo::from(private_key));
        }

        // Create storage client
        let client: StorageClient = StorageClient::new("url".to_string(), "key".to_string());

        // Fetch actived task from chain
        let pre_mock_executor_address: H160 =
            hex!("f60dB2d02af3f650798b59CB6D453b78f2C1BC90").into();
        let mut task = ActivedTaskFetcher {
            chain: Chain {
                id: 0,
                name: String::from("Ethereum"),
                chain_type: ChainType::Evm,
                endpoint: String::from(
                    "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
                ),
                native_asset: vec![0],
                foreign_asset: None,
                handler_contract: "0x056C0E37d026f9639313C281250cA932C9dbe921".into(),
                tx_indexer_url: Default::default(),
            },
            worker: AccountInfo {
                account20: pre_mock_executor_address.into(),
                account32: [0; 32],
            },
        }
        .fetch_task(&client)
        .unwrap()
        .unwrap();

        // Init task
        assert_eq!(task.init(
            &Context {
                signer: [0; 32],
                registry: &Registry {
                    chains: vec![
                        Chain {
                            id: 1,
                            name: String::from("Khala"),
                            endpoint: String::from("http://127.0.0.1:39933"),
                            chain_type: ChainType::Sub,
                            native_asset: vec![0],
                            foreign_asset: None,
                            handler_contract: "0x056C0E37d026f9639313C281250cA932C9dbe921".into(),
                            tx_indexer_url: Default::default(),
                        },
                        Chain {
                            id: 2,
                            name: String::from("Ethereum"),
                            endpoint: String::from("https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2"),
                            chain_type: ChainType::Evm,
                            native_asset: vec![0],
                            foreign_asset: None,
                            handler_contract: "0x056C0E37d026f9639313C281250cA932C9dbe921".into(),
                            tx_indexer_url: Default::default(),
                        }
                    ],
                    assets: vec![],
                },
                worker_accounts: worker_accounts.clone(),
            },
            &client,
        ), Ok(()));

        // Wait 3 seconds
        std::thread::sleep(std::time::Duration::from_millis(3000));

        // Now let's query if the task is exist in rollup storage with another rollup client
        let another_client: StorageClient =
            StorageClient::new("another url".to_string(), "key".to_string());
        let onchain_task = another_client.read::<Task>(&task.id).unwrap().unwrap().0;
        assert_eq!(onchain_task.status, TaskStatus::Initialized);
        assert_eq!(
            onchain_task.worker,
            worker_accounts.last().unwrap().account32
        );
    }

    fn build_multi_steps() -> Vec<MultiStep> {
        vec![
            MultiStep::Batch(vec![
                // moonbeam_stellaswap
                StepInput {
                    exe: String::from("moonbeam_stellaswap"),
                    source_chain: String::from("Moonbeam"),
                    dest_chain: String::from("Moonbeam"),
                    spend_asset: String::from("0xAcc15dC74880C9944775448304B263D191c6077F"),
                    receive_asset: String::from("0xFfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080"),
                    recipient: String::from("0xB8D20dfb8c3006AA17579887ABF719DA8bDf005B"),
                }
                .try_into()
                .unwrap(),
                // moonbeam_stellaswap
                StepInput {
                    exe: String::from("moonbeam_stellaswap"),
                    source_chain: String::from("Moonbeam"),
                    dest_chain: String::from("Moonbeam"),
                    spend_asset: String::from("0xFfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080"),
                    receive_asset: String::from("0xFFFfFfFf63d24eCc8eB8a7b5D0803e900F7b6cED"),
                    recipient: String::from("0xB8D20dfb8c3006AA17579887ABF719DA8bDf005B"),
                }
                .try_into()
                .unwrap(),
                // moonbeam_bridge_to_phala
                StepInput {
                    exe: String::from("moonbeam_bridge_to_phala"),
                    source_chain: String::from("Moonbeam"),
                    dest_chain: String::from("Phala"),
                    spend_asset: String::from("0xFFFfFfFf63d24eCc8eB8a7b5D0803e900F7b6cED"),
                    receive_asset: String::from("0x0000"),
                    recipient: String::from(
                        "0x1111111111111111111111111111111111111111111111111111111111111111",
                    ),
                }
                .try_into()
                .unwrap(),
            ]),
            // phala_bridge_to_astar
            MultiStep::Single(
                StepInput {
                    exe: String::from("phala_bridge_to_astar"),
                    source_chain: String::from("Phala"),
                    dest_chain: String::from("Astar"),
                    spend_asset: String::from("0x0000"),
                    receive_asset: String::from("0x010100cd1f"),
                    recipient: String::from(
                        "0x1111111111111111111111111111111111111111111111111111111111111111",
                    ),
                }
                .try_into()
                .unwrap(),
            ),
            MultiStep::Single(
                // astar_bridge_to_astar_evm
                StepInput {
                    exe: String::from("astar_bridge_to_astarevm"),
                    source_chain: String::from("Astar"),
                    dest_chain: String::from("AstarEvm"),
                    spend_asset: String::from("0x010100cd1f"),
                    receive_asset: String::from("0xFFFFFFFF00000000000000010000000000000006"),
                    recipient: String::from("0xbEA1C40ecf9c4603ec25264860B9b6623Ff733F5"),
                }
                .try_into()
                .unwrap(),
            ),
            MultiStep::Batch(vec![
                // astar_arthswap
                StepInput {
                    exe: String::from("astar_evm_arthswap"),
                    source_chain: String::from("AstarEvm"),
                    dest_chain: String::from("AstarEvm"),
                    spend_asset: String::from("0xFFFFFFFF00000000000000010000000000000006"),
                    receive_asset: String::from("0xFFfFfFffFFfffFFfFFfFFFFFffFFFffffFfFFFfF"),
                    recipient: String::from("0xbEA1C40ecf9c4603ec25264860B9b6623Ff733F5"),
                }
                .try_into()
                .unwrap(),
                // astar_arthswap
                StepInput {
                    exe: String::from("astar_evm_arthswap"),
                    source_chain: String::from("AstarEvm"),
                    dest_chain: String::from("AstarEvm"),
                    spend_asset: String::from("0xFFfFfFffFFfffFFfFFfFFFFFffFFFffffFfFFFfF"),
                    receive_asset: String::from("0xFFFFFFFF00000000000000010000000000000003"),
                    recipient: String::from("0xA29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20"),
                }
                .try_into()
                .unwrap(),
            ]),
        ]
    }

    #[test]
    fn test_calldata_generation() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let merged_steps = build_multi_steps();

        let worker_key = [0x11; 32];
        let mut task = Task {
            id: [1; 32],
            worker: AccountInfo::from(worker_key).account32,
            status: TaskStatus::Actived,
            source: "Moonbeam".to_string(),
            amount: 0xf0f1f2f3f4f5f6f7f8f9,
            fee: None,
            claim_nonce: None,
            claim_tx: None,
            merged_steps: merged_steps.clone(),
            execute_txs: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: hex::decode("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").unwrap(),
            retry_counter: 0,
        };
        let context = Context {
            signer: worker_key,
            worker_accounts: vec![AccountInfo::from(worker_key)],
            registry: &Registry::new(),
        };

        let mut calls = vec![];

        for step in task.merged_steps.iter_mut() {
            // Simulate settlement balance update
            step.set_spend(0xf0f1f2f3f4f5f6f7f8f9);
            calls.append(&mut step.derive_calls(&context).unwrap());
        }
        assert_eq!(calls.len(), 3 + 1 + 1 + 2);

        // Origin Step means Steps before merge

        // ========== First Merged Step =============
        // calls[0] build according to origin Step 0,
        // and origin Step 0 don't relay any previous steps happened
        // on the same chain
        assert_eq!(calls[0].input_call, Some(0));
        // calls[1] build according to origin Step 1,
        // and origin Step 1 relay Step 0 as input, so take last call
        // of Step 0 as input call
        assert_eq!(calls[1].input_call, Some(0));
        // calls[2] build according to origin Step 2,
        // and origin Step 2 relay Step 1 as input, so take last call
        // of Step 1 as input call
        assert_eq!(calls[2].input_call, Some(1));

        // ========== Second Merged Step =============
        // calls[5] build according to origin Step 5,
        // and origin Step 5 don't relay any previous steps happened
        // on the same chain
        assert_eq!(calls[5].input_call, Some(0));
        // calls[6] build according to origin Step 6,
        // and origin Step 6 relay Step 5 as input, so take last call
        // of Step 5 as input call
        assert_eq!(calls[6].input_call, Some(0));
    }
}
