use super::account::AccountInfo;
use super::context::Context;
use super::traits::Runner;
use crate::chain::{Chain, ChainType, NonceFetcher};
use crate::step::{MultiStep, Step};
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
    types::H160,
};

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum TaskStatus {
    /// Task initial confirmed by user on source chain.
    Actived,
    /// Task has been initialized, e.g. being applied nonce.
    Initialized,
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
    // Nonce applied to claim task froom source chain
    pub claim_nonce: Option<u64>,
    /// All steps to included in the task
    pub steps: Vec<Step>,
    /// Steps  after merged, those actually will be executed
    pub merged_steps: Vec<MultiStep>,
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
            claim_nonce: None,
            steps: vec![],
            merged_steps: vec![],
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
            .field("steps", &self.steps)
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
        let (mut free_accounts, free_accounts_doc) = client
            .read::<Vec<[u8; 32]>>(b"free_accounts")?
            .ok_or("StorageNotConfigured")?;
        let (mut pending_tasks, pending_tasks_doc) = client
            .read::<Vec<TaskId>>(b"pending_tasks")?
            .ok_or("StorageNotConfigured")?;

        pink_extension::debug!(
            "Trying to lookup storage for task {:?} before initializing.",
            hex::encode(self.id),
        );
        // if client.read_storage::<Task>(&self.id)?.is_some() {
        //     if !(self.has_claimed(context))? {
        //         pink_extension::debug!(
        //             "Task {:?} already exist in storage, but hasn't been claimed, try claim it with worker {:?} and return.",
        //             hex::encode(self.id),
        //             hex::encode(self.worker),
        //         );
        //         self.claim(context)?;
        //     }
        //     return Ok(());
        // }

        // Lookup free worker list to find if the worker we expected is free, if it's free remove it or return error
        if let Some(index) = free_accounts.iter().position(|&x| x == self.worker) {
            free_accounts.remove(index);
            pink_extension::debug!(
                "Worker {:?} is free, will be applied to this task {:?}.",
                hex::encode(self.worker),
                hex::encode(self.id)
            );
        } else {
            pink_extension::debug!(
                "Worker {:?} is busy, try again later for this task {:?}.",
                hex::encode(self.worker),
                hex::encode(self.id)
            );
            return Err("WorkerIsBusy");
        }

        // Apply recipient for each step before merged
        self.apply_recipient(context)?;

        // Merge steps
        self.merge_step(context)?;

        // Apply worker nonce for each step in task
        self.apply_nonce(0, context, client)?;

        // TODO: query initial balance of worker account and setup to specific step
        self.status = TaskStatus::Initialized;
        self.execute_index = 0;
        // Push to pending tasks queue
        pending_tasks.push(self.id);
        // Save task data
        client.insert(self.id.as_ref(), &self.encode())?;

        client.update(
            b"free_accounts".as_ref(),
            &free_accounts.encode(),
            free_accounts_doc,
        )?;
        client.update(
            b"pending_tasks".as_ref(),
            &pending_tasks.encode(),
            pending_tasks_doc,
        )?;

        // self.claim(context)?;

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
            self.claim(context)?;
            return Ok(self.status.clone());
        }

        let step_count = self.merged_steps.len();
        // To avoid unnecessary remote check, we check execute_index in advance
        if self.execute_index as usize == step_count {
            return Ok(TaskStatus::Completed);
        }

        match self.merged_steps[self.execute_index as usize].check(
            // An executing task must have nonce applied
            self.merged_steps[self.execute_index as usize]
                .get_nonce()
                .unwrap(),
            context,
        ) {
            // If step already executed successfully, execute next step
            Ok(true) => {
                self.execute_index += 1;
                self.retry_counter = 0;
                // If all step executed successfully, set task as `Completed`
                if self.execute_index as usize == step_count {
                    self.status = TaskStatus::Completed;
                    return Ok(self.status.clone());
                }

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
                self.retry_counter += 1;
                if self.retry_counter < 5 {
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

        if self.merged_steps[self.execute_index as usize].runnable(nonce, context, Some(client))
            == Ok(true)
        {
            pink_extension::debug!(
                "Trying to run step[{:?}] with nonce {:?}",
                self.execute_index,
                nonce
            );
            self.merged_steps[self.execute_index as usize].run(nonce, context)?;
            self.status = TaskStatus::Executing(self.execute_index, Some(nonce));
        } else {
            pink_extension::debug!("Step[{:?}] not runnable, return", self.execute_index);
        }
        Ok(self.status.clone())
    }

    /// Delete task record from on-chain storage
    pub fn destroy(&mut self, client: &StorageClient) -> Result<(), &'static str> {
        let (mut free_accounts, free_accounts_doc) = client
            .read::<Vec<[u8; 32]>>(b"free_accounts")?
            .ok_or("StorageNotConfigured")?;
        let (mut pending_tasks, pending_tasks_doc) = client
            .read::<Vec<TaskId>>(b"pending_tasks")?
            .ok_or("StorageNotConfigured")?;

        if let Some((_, task_doc)) = client.read::<Task>(&self.id)? {
            if let Some(idx) = pending_tasks.iter().position(|id| *id == self.id) {
                // Remove from pending tasks queue
                pending_tasks.remove(idx);
                // Recycle worker account
                free_accounts.push(self.worker);
                // Delete task data
                client.delete(self.id.as_ref(), task_doc)?;
            }
            client.update(
                b"free_accounts".as_ref(),
                &free_accounts.encode(),
                free_accounts_doc,
            )?;
            client.update(
                b"pending_tasks".as_ref(),
                &pending_tasks.encode(),
                pending_tasks_doc,
            )?;
        }

        Ok(())
    }

    pub fn apply_recipient(&mut self, context: &Context) -> Result<(), &'static str> {
        let step_count = self.steps.len();
        for (index, step) in self.steps.iter_mut().enumerate() {
            let step_source_chain = &step.source_chain(context).ok_or("MissingChain")?;
            let step_dest_chain = &step.dest_chain(context).ok_or("MissingChain")?;
            let worker_account_info = context.get_account(self.worker).ok_or("WorkerNotFound")?;

            // For sure last step we should put real recipient, or else the recipient could be either
            // worker account or handler account
            step.recipient = if index == step_count - 1 {
                Some(self.recipient.clone())
            } else {
                // If bridge to a EVM chain, asset should be send to Handler account to execute the reset of calls
                if step.is_bridge_step() {
                    if step_dest_chain.is_evm_chain() {
                        Some(step_dest_chain.handler_contract.clone())
                    } else {
                        Some(worker_account_info.account32.to_vec())
                    }
                } else {
                    // For non-bridge operatoions, because we don't batch call in Sub chains, so recipient should
                    // be worker account on source chain, or should be Handler address on source chain
                    if step_source_chain.is_sub_chain() {
                        let worker_account = match step_source_chain.chain_type {
                            ChainType::Evm => worker_account_info.account20.to_vec(),
                            ChainType::Sub => worker_account_info.account32.to_vec(),
                        };
                        Some(worker_account)
                    } else {
                        Some(step_source_chain.handler_contract.clone())
                    }
                }
            };
        }
        Ok(())
    }

    pub fn merge_step(&mut self, context: &Context) -> Result<(), &'static str> {
        let mut merged_steps: Vec<MultiStep> = vec![];
        let mut batch_steps: Vec<Step> = vec![];

        for (index, step) in self.steps.iter().enumerate() {
            let step_source_chain = &step.source_chain(context).ok_or("MissingChain")?;

            if step_source_chain.is_sub_chain() {
                if !batch_steps.is_empty() {
                    merged_steps.push(MultiStep::Batch(batch_steps.clone()));
                }
                merged_steps.push(MultiStep::Single(step.clone()));
                // clear queue
                batch_steps = vec![];
            } else {
                if batch_steps.is_empty() {
                    batch_steps.push(step.clone());
                } else {
                    // EVM chain hasn't changed
                    if step_source_chain.name.to_lowercase()
                        == batch_steps[batch_steps.len() - 1]
                            .source_chain
                            .to_lowercase()
                    {
                        batch_steps.push(step.clone())
                    }
                    // EVM chain changed
                    else {
                        // Push batch step
                        merged_steps.push(MultiStep::Batch(batch_steps.clone()));
                        // Reshipment batch step
                        batch_steps = vec![step.clone()];
                    }
                }
                // Save it if this is the last step
                if index == self.steps.len() - 1 {
                    merged_steps.push(MultiStep::Batch(batch_steps.clone()));
                }
            }
        }

        // Replace existing task steps
        self.merged_steps = merged_steps;

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
            if tx::check_tx(&chain.tx_indexer_url, &account, claim_nonce)? {
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

        // We call claimAndBatchCall so that first step will be executed along with the claim operation
        let first_step = &mut self.merged_steps[0];
        first_step.set_spend(self.amount);
        first_step.sync_origin_balance(context)?;
        let params = (task_id, first_step.derive_calls(context)?);
        pink_extension::info!("claimAndBatchCall params {:?}", &params);

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
                opt.gas = Some(gas);
                opt.nonce = Some(nonce.into());
            }),
            worker,
        ))
        .map_err(|e| {
            pink_extension::error!("claimAndBatchCall: failed to submit tx with error {:?}", &e);
            "ClaimSubmitFailed"
        })?;

        // Merge nonce to let check for first step work properly
        first_step.set_nonce(self.claim_nonce.unwrap());

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
        Ok(tx_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountInfo;
    use crate::chain::{BalanceFetcher, Chain, ChainType};
    use crate::registry::Registry;
    use crate::step::StepJson;
    use crate::task_fetcher::ActivedTaskFetcher;
    use crate::utils::ToArray;
    use dotenv::dotenv;
    use hex_literal::hex;
    use pink_web3::contract::tokens::Tokenize;
    use primitive_types::H160;

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
                tx_indexer_url: Default::default(),
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
        assert_eq!(
            task.steps[1].spend_amount.unwrap(),
            100_000_000_000_000_000_000 as u128
        );
        assert_eq!(task.steps[2].spend_amount.unwrap(), 12_000_000);
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
                tx_indexer_url: Default::default(),
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
        assert_eq!(task.steps[1].spend_amount.unwrap(), 301_000_000_000_000);
        assert_eq!(
            task.steps[2].spend_amount.unwrap(),
            1_000_000_000_000_000_000 as u128
        );
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
        // Setup initial worker accounts to storage
        let accounts: Vec<[u8; 32]> = worker_accounts
            .clone()
            .into_iter()
            .map(|account| account.account32.clone())
            .collect();
        client.insert(b"free_accounts", &accounts.encode()).unwrap();

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
        .fetch_task()
        .unwrap()
        .unwrap();
        assert_eq!(task.steps.len(), 3);

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
                },
                worker_accounts: worker_accounts.clone(),
            },
            &client,
        ), Ok(()));

        // Wait 3 seconds
        std::thread::sleep(std::time::Duration::from_millis(3000));

        // Now let's query if the task is exist in rollup storage with another rollup client
        let another_client = StorageClient::new("another url".to_string(), "key".to_string());
        let onchain_task = another_client.read::<Task>(&task.id).unwrap().unwrap().0;
        assert_eq!(onchain_task.status, TaskStatus::Initialized);
        assert_eq!(
            onchain_task.worker,
            worker_accounts.last().unwrap().account32
        );
        assert_eq!(onchain_task.steps.len(), 3);
        assert_eq!(onchain_task.steps[0].nonce, Some(0));
        assert_eq!(onchain_task.steps[1].nonce, Some(1));
        assert_eq!(onchain_task.steps[2].nonce, Some(2));
    }

    fn build_steps() -> Vec<Step> {
        vec![
            // moonbeam_stellaswap
            StepJson {
                exe_type: String::from("swap"),
                exe: String::from("moonbeam_stellaswap"),
                source_chain: String::from("Moonbeam"),
                dest_chain: String::from("Moonbeam"),
                spend_asset: String::from("0xAcc15dC74880C9944775448304B263D191c6077F"),
                receive_asset: String::from("0xFfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080"),
            }
            .try_into()
            .unwrap(),
            // moonbeam_stellaswap
            StepJson {
                exe_type: String::from("swap"),
                exe: String::from("moonbeam_stellaswap"),
                source_chain: String::from("Moonbeam"),
                dest_chain: String::from("Moonbeam"),
                spend_asset: String::from("0xFfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080"),
                receive_asset: String::from("0xFFFfFfFf63d24eCc8eB8a7b5D0803e900F7b6cED"),
            }
            .try_into()
            .unwrap(),
            // moonbeam_bridge_to_phala
            StepJson {
                exe_type: String::from("bridge"),
                exe: String::from("moonbeam_bridge_to_phala"),
                source_chain: String::from("Moonbeam"),
                dest_chain: String::from("Phala"),
                spend_asset: String::from("0xFFFfFfFf63d24eCc8eB8a7b5D0803e900F7b6cED"),
                receive_asset: String::from("0x0000"),
            }
            .try_into()
            .unwrap(),
            // phala_bridge_to_astar
            StepJson {
                exe_type: String::from("bridge"),
                exe: String::from("phala_bridge_to_astar"),
                source_chain: String::from("Phala"),
                dest_chain: String::from("Astar"),
                spend_asset: String::from("0x0000"),
                receive_asset: String::from("0x010100cd1f"),
            }
            .try_into()
            .unwrap(),
            // astar_bridge_to_astar_evm
            StepJson {
                exe_type: String::from("bridge"),
                exe: String::from("astar_bridge_to_astarevm"),
                source_chain: String::from("Astar"),
                dest_chain: String::from("AstarEvm"),
                spend_asset: String::from("0x010100cd1f"),
                receive_asset: String::from("0xFFFFFFFF00000000000000010000000000000006"),
            }
            .try_into()
            .unwrap(),
            // astar_arthswap
            StepJson {
                exe_type: String::from("swap"),
                exe: String::from("astar_evm_arthswap"),
                source_chain: String::from("AstarEvm"),
                dest_chain: String::from("AstarEvm"),
                spend_asset: String::from("0xFFFFFFFF00000000000000010000000000000006"),
                receive_asset: String::from("0xFFfFfFffFFfffFFfFFfFFFFFffFFFffffFfFFFfF"),
            }
            .try_into()
            .unwrap(),
            // astar_arthswap
            StepJson {
                exe_type: String::from("swap"),
                exe: String::from("astar_evm_arthswap"),
                source_chain: String::from("AstarEvm"),
                dest_chain: String::from("AstarEvm"),
                spend_asset: String::from("0xFFfFfFffFFfffFFfFFfFFFFFffFFFffffFfFFFfF"),
                receive_asset: String::from("0xFFFFFFFF00000000000000010000000000000003"),
            }
            .try_into()
            .unwrap(),
        ]
    }

    #[test]
    fn test_apply_recipient() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let worker_key = [0x11; 32];
        let steps = build_steps();
        let mut task = Task {
            id: [1; 32],
            worker: AccountInfo::from(worker_key).account32,
            status: TaskStatus::Actived,
            source: "Moonbeam".to_string(),
            amount: 0,
            claim_nonce: None,
            steps,
            merged_steps: vec![],
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

        task.apply_recipient(&context).unwrap();

        // moonbeam_stellaswap
        assert_eq!(
            task.steps[0].recipient,
            Some(
                context
                    .registry
                    .get_chain(&String::from("Moonbeam"))
                    .unwrap()
                    .handler_contract
            )
        );
        // moonbeam_stellaswap
        assert_eq!(
            task.steps[1].recipient,
            Some(
                context
                    .registry
                    .get_chain(&String::from("Moonbeam"))
                    .unwrap()
                    .handler_contract
            )
        );
        // moonbeam_bridge_to_phala
        assert_eq!(
            task.steps[2].recipient,
            Some(AccountInfo::from(worker_key).account32.to_vec())
        );
        // phala_bridge_to_astar
        assert_eq!(
            task.steps[3].recipient,
            Some(AccountInfo::from(worker_key).account32.to_vec())
        );
        // astar_bridge_to_astar_evm
        assert_eq!(
            task.steps[4].recipient,
            Some(
                context
                    .registry
                    .get_chain(&String::from("AstarEvm"))
                    .unwrap()
                    .handler_contract
            )
        );
        // astar_arthswap
        assert_eq!(
            task.steps[5].recipient,
            Some(
                context
                    .registry
                    .get_chain(&String::from("AstarEvm"))
                    .unwrap()
                    .handler_contract
            )
        );
        // astar_arthswap
        assert_eq!(task.steps[6].recipient, Some(task.recipient));
    }

    #[test]
    fn test_merge_step() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let worker_key = [0x11; 32];
        let steps = build_steps();
        let mut task = Task {
            id: [1; 32],
            worker: AccountInfo::from(worker_key).account32,
            status: TaskStatus::Actived,
            source: "Moonbeam".to_string(),
            amount: 0,
            claim_nonce: None,
            steps: steps.clone(),
            merged_steps: vec![],
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

        task.apply_recipient(&context).unwrap();
        task.merge_step(&context).unwrap();

        assert_eq!(task.merged_steps.len(), 4);
        assert!(task.merged_steps[0].is_batch_step());
        match &task.merged_steps[0] {
            MultiStep::Batch(batch_steps) => {
                assert_eq!(batch_steps.len(), 3);
                assert_eq!(batch_steps[0].source_chain, "Moonbeam");
                assert_eq!(batch_steps[1].source_chain, "Moonbeam");
                assert_eq!(batch_steps[2].dest_chain, "Phala");
            }
            _ => assert!(false),
        };
        assert!(task.merged_steps[1].is_single_step());
        match &task.merged_steps[1] {
            MultiStep::Single(step) => {
                assert_eq!(step.source_chain, "Phala");
                assert_eq!(step.dest_chain, "Astar");
            }
            _ => assert!(false),
        };
        assert!(task.merged_steps[2].is_single_step());
        match &task.merged_steps[2] {
            MultiStep::Single(step) => {
                assert_eq!(step.source_chain, "Astar");
                assert_eq!(step.dest_chain, "AstarEvm");
            }
            _ => assert!(false),
        };
        assert!(task.merged_steps[3].is_batch_step());
        match &task.merged_steps[3] {
            MultiStep::Batch(batch_steps) => {
                assert_eq!(batch_steps.len(), 2);
                assert_eq!(batch_steps[0].source_chain, "AstarEvm");
                assert_eq!(batch_steps[1].source_chain, "AstarEvm");
            }
            _ => assert!(false),
        };

        let mut task1 = Task {
            id: [1; 32],
            worker: AccountInfo::from(worker_key).account32,
            status: TaskStatus::Actived,
            source: "Moonbeam".to_string(),
            amount: 0,
            claim_nonce: None,
            steps: steps.clone().as_slice()[3..].to_vec(),
            merged_steps: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: hex::decode("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").unwrap(),
            retry_counter: 0,
        };

        task1.apply_recipient(&context).unwrap();
        task1.merge_step(&context).unwrap();
        assert_eq!(task1.merged_steps.len(), 3);
        assert!(task1.merged_steps[0].is_single_step());
        match &task1.merged_steps[0] {
            MultiStep::Single(step) => {
                assert_eq!(step.source_chain, "Phala");
                assert_eq!(step.dest_chain, "Astar");
            }
            _ => assert!(false),
        };
        assert!(task1.merged_steps[1].is_single_step());
        match &task1.merged_steps[1] {
            MultiStep::Single(step) => {
                assert_eq!(step.source_chain, "Astar");
                assert_eq!(step.dest_chain, "AstarEvm");
            }
            _ => assert!(false),
        };
        assert!(task1.merged_steps[2].is_batch_step());
        match &task1.merged_steps[2] {
            MultiStep::Batch(batch_steps) => {
                assert_eq!(batch_steps.len(), 2);
                assert_eq!(batch_steps[0].source_chain, "AstarEvm");
                assert_eq!(batch_steps[1].source_chain, "AstarEvm");
            }
            _ => assert!(false),
        };

        let mut task2 = Task {
            id: [1; 32],
            worker: AccountInfo::from(worker_key).account32,
            status: TaskStatus::Actived,
            source: "Moonbeam".to_string(),
            amount: 0,
            claim_nonce: None,
            steps: steps.clone().as_slice()[..4].to_vec(),
            merged_steps: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: hex::decode("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").unwrap(),
            retry_counter: 0,
        };

        task2.apply_recipient(&context).unwrap();
        task2.merge_step(&context).unwrap();

        assert_eq!(task2.merged_steps.len(), 2);
        assert!(task2.merged_steps[0].is_batch_step());
        match &task2.merged_steps[0] {
            MultiStep::Batch(batch_steps) => {
                assert_eq!(batch_steps.len(), 3);
                assert_eq!(batch_steps[0].source_chain, "Moonbeam");
                assert_eq!(batch_steps[1].source_chain, "Moonbeam");
                assert_eq!(batch_steps[2].dest_chain, "Phala");
            }
            _ => assert!(false),
        };
        assert!(task2.merged_steps[1].is_single_step());
        match &task2.merged_steps[1] {
            MultiStep::Single(step) => {
                assert_eq!(step.source_chain, "Phala");
                assert_eq!(step.dest_chain, "Astar");
            }
            _ => assert!(false),
        };

        let mut task3 = Task {
            id: [1; 32],
            worker: AccountInfo::from(worker_key).account32,
            status: TaskStatus::Actived,
            source: "Moonbeam".to_string(),
            amount: 0,
            claim_nonce: None,
            steps: [
                &steps.clone().as_slice()[..3],
                &steps.clone().as_slice()[5..],
            ]
            .concat()
            .to_vec(),
            merged_steps: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: hex::decode("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").unwrap(),
            retry_counter: 0,
        };

        task3.apply_recipient(&context).unwrap();
        task3.merge_step(&context).unwrap();

        assert_eq!(task3.merged_steps.len(), 2);
        assert!(task3.merged_steps[0].is_batch_step());
        match &task3.merged_steps[0] {
            MultiStep::Batch(batch_steps) => {
                assert_eq!(batch_steps.len(), 3);
                assert_eq!(batch_steps[0].source_chain, "Moonbeam");
                assert_eq!(batch_steps[1].source_chain, "Moonbeam");
                assert_eq!(batch_steps[2].dest_chain, "Phala");
            }
            _ => assert!(false),
        };
        assert!(task3.merged_steps[1].is_batch_step());
        match &task3.merged_steps[1] {
            MultiStep::Batch(batch_steps) => {
                assert_eq!(batch_steps.len(), 2);
                assert_eq!(batch_steps[0].source_chain, "AstarEvm");
                assert_eq!(batch_steps[1].source_chain, "AstarEvm");
            }
            _ => assert!(false),
        };
    }

    #[test]
    fn test_calldata_generation() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let worker_key = [0x11; 32];
        let steps = build_steps();
        let mut task = Task {
            id: [1; 32],
            worker: AccountInfo::from(worker_key).account32,
            status: TaskStatus::Actived,
            source: "Moonbeam".to_string(),
            amount: 0xf0f1f2f3f4f5f6f7f8f9,
            claim_nonce: None,
            steps: steps.clone(),
            merged_steps: vec![],
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

        task.apply_recipient(&context).unwrap();
        task.merge_step(&context).unwrap();

        let mut calls = vec![];

        for step in task.merged_steps.iter_mut() {
            // Simulate settlement balance update
            step.set_spend(0xf0f1f2f3f4f5f6f7f8f9);
            calls.append(&mut step.derive_calls(&context).unwrap());
        }
        assert_eq!(calls.len(), 2 * 3 + 1 + 1 + 2 * 2);

        // Origin Step means Steps before merge

        // ========== First Merged Step =============
        // calls[0] and calls[1] build according to origin Step 0,
        // and origin Step 0 don't relay any previous steps happened
        // on the same chain
        assert_eq!(calls[0].input_call, Some(0));
        assert_eq!(calls[1].input_call, Some(0));
        // calls[2] and calls[3] build according to origin Step 1,
        // and origin Step 1 relay Step 0 as input, so take last call
        // of Step 0 as input call
        assert_eq!(calls[2].input_call, Some(1));
        assert_eq!(calls[3].input_call, Some(1));
        // calls[4] and calls[5] build according to origin Step 2,
        // and origin Step 2 relay Step 1 as input, so take last call
        // of Step 1 as input call
        assert_eq!(calls[4].input_call, Some(3));
        assert_eq!(calls[5].input_call, Some(3));

        // ========== Second Merged Step =============
        // calls[0] and calls[1] build according to origin Step 5,
        // and origin Step 5 don't relay any previous steps heppened
        // on the same chain
        assert_eq!(calls[0].input_call, Some(0));
        assert_eq!(calls[1].input_call, Some(0));
        // calls[2] and calls[3] build according to origin Step 6,
        // and origin Step 6 relay Step 5 as input, so take last call
        // of Step 5 as input call
        assert_eq!(calls[2].input_call, Some(1));
        assert_eq!(calls[3].input_call, Some(1));
    }

    #[test]
    fn test_calldata() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        use crate::call::{Call, CallParams, EvmCall};
        use pink_web3::types::{Address, U256};

        let handler: H160 =
            H160::from_slice(&hex::decode("B30A27eE79514614dc363CE0aABb0B939b9deAeD").unwrap());
        let transport = Eth::new(PinkHttp::new("https://rpc.api.moonbeam.network"));
        let handler =
            Contract::from_json(transport, handler, include_bytes!("./abi/handler.json")).unwrap();
        let task_id: [u8; 32] =
            hex::decode("1125000000000000000000000000000000000000000000000000000000000000")
                .unwrap()
                .to_array();
        // We call claimAndBatchCall so that first step will be executed along with the claim operation

        let params = (
            task_id,
            vec![
                Call {
                    params: CallParams::Evm(EvmCall {
                        target: Address::from_slice(
                            hex::decode("acc15dc74880c9944775448304b263d191c6077f")
                                .unwrap()
                                .as_slice(),
                        ),
                        calldata: vec![
                            9, 94, 167, 179, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 112, 8, 90, 9,
                            211, 13, 111, 140, 78, 207, 110, 225, 1, 32, 209, 132, 115, 131, 187,
                            87, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 13, 224, 182, 179, 167, 100, 0, 0,
                        ],
                        value: U256::from(0),

                        need_settle: false,
                        update_offset: U256::from(36),
                        update_len: U256::from(32),
                        spend_asset: Address::from_slice(
                            hex::decode("acc15dc74880c9944775448304b263d191c6077f")
                                .unwrap()
                                .as_slice(),
                        ),
                        spend_amount: U256::from(1000000000000000000_u128),
                        receive_asset: Address::from_slice(
                            hex::decode("acc15dc74880c9944775448304b263d191c6077f")
                                .unwrap()
                                .as_slice(),
                        ),
                    }),
                    input_call: Some(0),
                    call_index: Some(0),
                },
                Call {
                    params: CallParams::Evm(EvmCall {
                        target: Address::from_slice(
                            hex::decode("70085a09d30d6f8c4ecf6ee10120d1847383bb57")
                                .unwrap()
                                .as_slice(),
                        ),
                        calldata: vec![
                            56, 237, 23, 57, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 13, 224, 182, 179, 167, 100, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 160, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99,
                            94, 168, 104, 4, 32, 15, 128, 193, 110, 168, 237, 220, 60, 116, 154,
                            84, 169, 195, 125, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 101, 14, 240, 187, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 172, 193, 93, 199, 72, 128,
                            201, 148, 71, 117, 68, 131, 4, 178, 99, 209, 145, 198, 7, 127, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 31, 202, 203, 210, 24,
                            237, 192, 235, 162, 15, 194, 48, 140, 119, 128, 128,
                        ],
                        value: U256::from(0),

                        need_settle: true,
                        update_offset: U256::from(4),
                        update_len: U256::from(32),
                        spend_asset: Address::from_slice(
                            hex::decode("acc15dc74880c9944775448304b263d191c6077f")
                                .unwrap()
                                .as_slice(),
                        ),
                        spend_amount: U256::from(1000000000000000000_u128),
                        receive_asset: Address::from_slice(
                            hex::decode("ffffffff1fcacbd218edc0eba20fc2308c778080")
                                .unwrap()
                                .as_slice(),
                        ),
                    }),
                    input_call: Some(0),
                    call_index: Some(1),
                },
                Call {
                    params: CallParams::Evm(EvmCall {
                        target: Address::from_slice(
                            hex::decode("ffffffff1fcacbd218edc0eba20fc2308c778080")
                                .unwrap()
                                .as_slice(),
                        ),
                        calldata: vec![
                            9, 94, 167, 179, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 112, 8, 90, 9,
                            211, 13, 111, 140, 78, 207, 110, 225, 1, 32, 209, 132, 115, 131, 187,
                            87, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        ],
                        value: U256::from(0),

                        need_settle: false,
                        update_offset: U256::from(36),
                        update_len: U256::from(32),
                        spend_asset: Address::from_slice(
                            hex::decode("ffffffff1fcacbd218edc0eba20fc2308c778080")
                                .unwrap()
                                .as_slice(),
                        ),
                        spend_amount: U256::from(0),
                        receive_asset: Address::from_slice(
                            hex::decode("ffffffff1fcacbd218edc0eba20fc2308c778080")
                                .unwrap()
                                .as_slice(),
                        ),
                    }),
                    input_call: Some(1),
                    call_index: Some(2),
                },
                Call {
                    params: CallParams::Evm(EvmCall {
                        target: Address::from_slice(
                            hex::decode("70085a09d30d6f8c4ecf6ee10120d1847383bb57")
                                .unwrap()
                                .as_slice(),
                        ),
                        calldata: vec![
                            56, 237, 23, 57, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 160, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99, 94, 168,
                            104, 4, 32, 15, 128, 193, 110, 168, 237, 220, 60, 116, 154, 84, 169,
                            195, 125, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 101, 14, 240, 189, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 31, 202, 203, 210,
                            24, 237, 192, 235, 162, 15, 194, 48, 140, 119, 128, 128, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 99, 210, 78, 204, 142, 184,
                            167, 181, 208, 128, 62, 144, 15, 123, 108, 237,
                        ],
                        value: U256::from(0),

                        need_settle: true,
                        update_offset: U256::from(4),
                        update_len: U256::from(32),
                        spend_asset: Address::from_slice(
                            hex::decode("ffffffff1fcacbd218edc0eba20fc2308c778080")
                                .unwrap()
                                .as_slice(),
                        ),
                        spend_amount: U256::from(0),
                        receive_asset: Address::from_slice(
                            hex::decode("ffffffff63d24ecc8eb8a7b5d0803e900f7b6ced")
                                .unwrap()
                                .as_slice(),
                        ),
                    }),
                    input_call: Some(1),
                    call_index: Some(3),
                },
                Call {
                    params: CallParams::Evm(EvmCall {
                        target: Address::from_slice(
                            hex::decode("ffffffff63d24ecc8eb8a7b5d0803e900f7b6ced")
                                .unwrap()
                                .as_slice(),
                        ),
                        calldata: vec![
                            9, 94, 167, 179, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        ],
                        value: U256::from(0),

                        need_settle: false,
                        update_offset: U256::from(36),
                        update_len: U256::from(32),
                        spend_asset: Address::from_slice(
                            hex::decode("ffffffff63d24ecc8eb8a7b5d0803e900f7b6ced")
                                .unwrap()
                                .as_slice(),
                        ),
                        spend_amount: U256::from(0),
                        receive_asset: Address::from_slice(
                            hex::decode("ffffffff63d24ecc8eb8a7b5d0803e900f7b6ced")
                                .unwrap()
                                .as_slice(),
                        ),
                    }),
                    input_call: Some(3),
                    call_index: Some(4),
                },
                Call {
                    params: CallParams::Evm(EvmCall {
                        target: Address::from_slice(
                            hex::decode("0000000000000000000000000000000000000804")
                                .unwrap()
                                .as_slice(),
                        ),
                        calldata: vec![
                            185, 248, 19, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255,
                            255, 99, 210, 78, 204, 142, 184, 167, 181, 208, 128, 62, 144, 15, 123,
                            108, 237, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            1, 101, 160, 188, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 7, 243, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 34, 1, 4, 219, 160, 103, 127, 194, 116, 255, 172, 204, 15, 161,
                            3, 10, 102, 177, 113, 209, 218, 146, 38, 210, 187, 157, 21, 38, 84,
                            230, 167, 70, 242, 118, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        ],
                        value: U256::from(0),

                        need_settle: false,
                        update_offset: U256::from(36),
                        update_len: U256::from(32),
                        spend_asset: Address::from_slice(
                            hex::decode("ffffffff63d24ecc8eb8a7b5d0803e900f7b6ced")
                                .unwrap()
                                .as_slice(),
                        ),
                        spend_amount: U256::from(0),
                        receive_asset: Address::from_slice(
                            hex::decode("0000000000000000000000000000000000000000")
                                .unwrap()
                                .as_slice(),
                        ),
                    }),
                    input_call: Some(3),
                    call_index: Some(5),
                },
            ],
        );

        let claim_func = handler
            .abi()
            .function("claimAndBatchCall")
            .map_err(|_| "NoFunctionFound")
            .unwrap();
        let calldata = claim_func
            .encode_input(&params.into_tokens())
            .map_err(|_| "EncodeParamError")
            .unwrap();
        println!("claim calldata: {:?}", hex::encode(calldata));
    }
}
