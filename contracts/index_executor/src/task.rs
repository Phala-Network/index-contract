use super::account::AccountInfo;
use super::context::Context;
use super::traits::Runner;
use crate::chain::{Chain, ChainType, NonceFetcher};
use crate::step::{MultiStep, Step, StepJson};
use crate::storage::StorageClient;
use crate::tx;
use alloc::{string::String, vec, vec::Vec};
use ink::storage::Mapping;
use pink_extension::ResultExt;
use scale::{Decode, Encode};
use xcm::v2::AssetId as XcmAssetId;

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
        let execution_plan_json: ExecutionPlan =
            pink_json::from_str(&self.task).map_err(|_| "InvalidTask")?;
        pink_extension::debug!(
            "Parse task data successfully, found {:?} operations",
            execution_plan_json.len()
        );
        if execution_plan_json.is_empty() {
            return Err("EmptyTask");
        }
        pink_extension::debug!("Trying to convert task data to task");

        let mut uninitialized_task = Task {
            id,
            source: source_chain.into(),
            sender: self.sender.clone(),
            recipient: self.recipient.clone(),
            amount: self.amount,
            worker,
            ..Default::default()
        };

        for step_json in execution_plan_json.iter() {
            uninitialized_task.steps.push(step_json.clone().try_into()?);
        }

        Ok(uninitialized_task)
    }
}

type ExecutionPlan = Vec<StepJson>;

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

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
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

impl Task {
    // Initialize task
    pub fn init(&mut self, context: &Context, client: &StorageClient) -> Result<(), &'static str> {
        let (mut free_accounts, free_accounts_doc) = client
            .read_storage::<Vec<[u8; 32]>>(b"free_accounts")?
            .ok_or("StorageNotConfigured")?;
        let (mut pending_tasks, pending_tasks_doc) = client
            .read_storage::<Vec<TaskId>>(b"pending_tasks")?
            .ok_or("StorageNotConfigured")?;

        if client.read_storage::<Task>(&self.id)?.is_some() {
            if !(self.has_claimed(context))? {
                self.claim(context)?;
            }
            return Ok(());
        }

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

        // Apply nonce
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
        client.alloc_storage(self.id.as_ref(), &self.encode())?;

        client.update_storage(
            b"free_accounts".as_ref(),
            &free_accounts.encode(),
            free_accounts_doc,
        )?;
        client.update_storage(
            b"pending_tasks".as_ref(),
            &pending_tasks.encode(),
            pending_tasks_doc,
        )?;

        self.claim(context)?;

        Ok(())
    }

    pub fn execute(
        &mut self,
        context: &Context,
        client: &StorageClient,
    ) -> Result<TaskStatus, &'static str> {
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
            .read_storage::<Vec<[u8; 32]>>(b"free_accounts")?
            .ok_or("StorageNotConfigured")?;
        let (mut pending_tasks, pending_tasks_doc) = client
            .read_storage::<Vec<TaskId>>(b"pending_tasks")?
            .ok_or("StorageNotConfigured")?;

        if let Some((_, task_doc)) = client.read_storage::<Task>(&self.id)? {
            if let Some(idx) = pending_tasks.iter().position(|id| *id == self.id) {
                // Remove from pending tasks queue
                pending_tasks.remove(idx);
                // Recycle worker account
                free_accounts.push(self.worker);
                // Delete task data
                client.remove_storage(self.id.as_ref(), task_doc)?;
            }
            client.update_storage(
                b"free_accounts".as_ref(),
                &free_accounts.encode(),
                free_accounts_doc,
            )?;
            client.update_storage(
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

            // For sure last step we should put real recipient, or else the recipient could be either
            // worker account or handler account
            step.recipient = if index == step_count - 1 {
                Some(self.recipient.clone())
            } else {
                // If bridge to a EVM chain, asset should be send to Handler account to execute the reset of calls
                if step.is_bridge_step() && step_dest_chain.is_evm_chain() {
                    Some(step_dest_chain.handler_contract.clone())
                } else {
                    // For non-bridge operatoions, because we don't batch call in Sub chains, so recipient should
                    // be worker account on source chain, or should be Handler address on source chain
                    if step_source_chain.is_sub_chain() {
                        let account_info =
                            context.get_account(self.worker).ok_or("WorkerNotFound")?;
                        let worker_account = match step_source_chain.chain_type {
                            ChainType::Evm => account_info.account20.to_vec(),
                            ChainType::Sub => account_info.account32.to_vec(),
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

        for step in self.steps.iter() {
            let step_source_chain = &step.source_chain(context).ok_or("MissingChain")?;

            if step_source_chain.is_sub_chain() {
                merged_steps.push(MultiStep::Single(step.clone()));
            } else {
                if batch_steps.is_empty() {
                    batch_steps.push(step.clone())
                } else {
                    if step_source_chain.name.to_lowercase()
                        == batch_steps[batch_steps.len() - 1]
                            .source_chain
                            .to_lowercase()
                    {
                        batch_steps.push(step.clone())
                    } else {
                        // Push  batch step
                        merged_steps.push(MultiStep::Batch(batch_steps.clone()));
                        // Clear batch step
                        batch_steps = vec![step.clone()];
                    }
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

        // Apply nonce for claim operation
        let claim_nonce = self.get_nonce(context, &self.source)?;
        nonce_map.insert(self.source.clone(), &(claim_nonce + 1));
        self.claim_nonce = Some(claim_nonce);

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
            if tx::check_tx(&chain.tx_indexer, &account, claim_nonce)? {
                return Ok(true);
            } else {
                return Err("ClaimFailed");
            }
        } else {
            return Ok(false);
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
        let calls = first_step.derive_calls(context)?;
        first_step.sync_origin_balance(context)?;
        let params = (task_id, calls);
        // Estiamte gas before submission
        let gas = resolve_ready(handler.estimate_gas(
            "claimAndBatchCall",
            params.clone(),
            worker.address(),
            Options::default(),
        ))
        .map_err(|_| "GasEstimateFailed")?;

        // Submit the claim transaction
        let tx_id = resolve_ready(handler.signed_call(
            "claim",
            params,
            Options::with(|opt| {
                opt.gas = Some(gas);
                opt.nonce = Some(nonce.into());
            }),
            worker,
        ))
        .map_err(|_| "ClaimSubmitFailed")?;

        // Merge nonce to let check for first step work properly
        first_step.set_nonce(self.claim_nonce.unwrap());

        pink_extension::info!(
            "Submit transaction to claim task {:?} on ${:?}, tx id: {:?}",
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
            tx_indexer: Default::default(),
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
            tx_indexer: Default::default(),
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
            tx_indexer: Default::default(),
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
        client
            .alloc_storage(b"free_accounts", &accounts.encode())
            .unwrap();

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
                tx_indexer: Default::default(),
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
                            tx_indexer: Default::default(),
                        },
                        Chain {
                            id: 2,
                            name: String::from("Ethereum"),
                            endpoint: String::from("https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2"),
                            chain_type: ChainType::Evm,
                            native_asset: vec![0],
                            foreign_asset: None,
                            handler_contract: "0x056C0E37d026f9639313C281250cA932C9dbe921".into(),
                            tx_indexer: Default::default(),
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
        let onchain_task = another_client
            .read_storage::<Task>(&task.id)
            .unwrap()
            .unwrap()
            .0;
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
}
