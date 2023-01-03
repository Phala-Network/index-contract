use super::context::Context;
use super::step::Step;
use super::traits::Runner;
use alloc::{string::String, vec, vec::Vec};
use index::graph::ChainType;
use index::graph::NonceFetcher;
use ink_storage::Mapping;
use kv_session::traits::KvSession;
use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;
use scale::{Decode, Encode};

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
    /// All steps to included in the task
    pub steps: Vec<Step>,
    /// Current step index that is executing
    pub execute_index: u8,
    /// Sender address on source chain
    pub sender: Vec<u8>,
    /// Recipient address on dest chain
    pub recipient: Vec<u8>,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: [0; 32],
            worker: [0; 32],
            status: TaskStatus::Actived,
            source: String::default(),
            steps: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: vec![],
        }
    }
}

impl Task {
    // Initialize task
    pub fn init(&mut self, context: &Context, client: &mut SubstrateRollupClient) {
        let mut free_accounts = OnchainAccounts::lookup_free_accounts(client);
        let mut pending_tasks = OnchainTasks::lookup_pending_tasks(client);

        if OnchainTasks::lookup_task(client, &self.id).is_some() {
            // Task already saved, return
            return;
        }
        if let Some(account) = free_accounts.pop() {
            // Apply a worker account
            self.worker = account;
            // Aplly worker nonce for each step in task
            self.aplly_nonce(context, client);
            // TODO: query initial balance of worker account and setup to specific step
            self.status = TaskStatus::Initialized;
            self.execute_index = 0;
            // Push to pending tasks queue
            pending_tasks.push(self.id);
            // Save task data
            client.session().put(&self.id.to_vec(), self.encode());
        } else {
            // We can not handle more tasks any more
            return;
        }

        client
            .session()
            .put(&b"free_accounts".to_vec(), free_accounts.encode());
        client
            .session()
            .put(&b"pending_tasks".to_vec(), pending_tasks.encode());
    }

    // Recover execution status according to on-chain storage
    pub fn sync(&mut self, _client: &SubstrateRollupClient) {
        for step in self.steps.iter() {
            if step.check(step.nonce.unwrap()) {
                self.execute_index += 1;
                // If all step executed successfully, set task as `Completed`
                if self.execute_index as usize == self.steps.len() {
                    self.status = TaskStatus::Completed;
                    break;
                }
            } else {
                self.status = TaskStatus::Executing(self.execute_index, step.nonce);
                // Exit with current status
                break;
            }
        }
    }

    pub fn execute(&mut self, context: &Context) -> Result<TaskStatus, &'static str> {
        // If step already executed successfully, execute next step
        if self.steps[self.execute_index as usize]
            .check(self.steps[self.execute_index as usize].nonce.unwrap())
        {
            self.execute_index += 1;
            // If all step executed successfully, set task as `Completed`
            if self.execute_index as usize == self.steps.len() {
                self.status = TaskStatus::Completed;
                return Ok(self.status.clone());
            }
        }

        if self.steps[self.execute_index as usize].runnable() {
            self.steps[self.execute_index as usize].run(context)?;
            self.status = TaskStatus::Executing(
                self.execute_index,
                self.steps[self.execute_index as usize].nonce,
            );
        }
        Ok(self.status.clone())
    }

    /// Delete task record from on-chain storage
    pub fn destroy(&mut self, client: &mut SubstrateRollupClient) {
        let mut free_accounts = OnchainAccounts::lookup_free_accounts(client);
        let mut pending_tasks = OnchainTasks::lookup_pending_tasks(client);

        if OnchainTasks::lookup_task(client, &self.id).is_some() {
            if let Some(idx) = pending_tasks.iter().position(|id| *id == self.id) {
                // Remove from pending tasks queue
                pending_tasks.remove(idx);
                // Recycle worker account
                free_accounts.push(self.worker);
                // Delete task data
                client.session().delete(&self.id.to_vec());
            }
            client
                .session()
                .put(&b"free_accounts".to_vec(), free_accounts.encode());
            client
                .session()
                .put(&b"pending_tasks".to_vec(), pending_tasks.encode());
        }
    }

    fn aplly_nonce(&mut self, context: &Context, _client: &SubstrateRollupClient) {
        let mut nonce_map: Mapping<String, u64> = Mapping::default();
        for step in self.steps.iter_mut() {
            let nonce = nonce_map.get(&step.chain).or_else(|| {
                let chain = context.graph.get_chain(step.chain.clone()).unwrap();
                let account_info = context.get_account(self.worker).unwrap();
                let account = match chain.chain_type {
                    ChainType::Evm => account_info.account20.to_vec(),
                    ChainType::Sub => account_info.account32.to_vec(),
                    // ChainType::Unknown => panic!("chain not supported!"),
                };
                chain.get_nonce(account).ok()
            });
            step.nonce = nonce;
            // Increase nonce by 1
            nonce_map.insert(step.chain.clone(), &(nonce.unwrap() + 1));
        }
    }
}

pub struct OnchainTasks;
impl OnchainTasks {
    pub fn lookup_task(client: &mut SubstrateRollupClient, id: &TaskId) -> Option<Task> {
        if let Ok(Some(raw_task)) = client.session().get(&id.to_vec()) {
            return match Decode::decode(&mut raw_task.as_slice()) {
                Ok(task) => Some(task),
                Err(_) => None,
            };
        }
        None
    }

    pub fn lookup_pending_tasks(client: &mut SubstrateRollupClient) -> Vec<TaskId> {
        if let Ok(Some(raw_tasks)) = client.session().get(&b"pending_tasks".to_vec()) {
            return match Decode::decode(&mut raw_tasks.as_slice()) {
                Ok(tasks) => tasks,
                Err(_) => vec![],
            };
        }
        vec![]
    }
}

pub struct OnchainAccounts;
impl OnchainAccounts {
    pub fn lookup_free_accounts(client: &mut SubstrateRollupClient) -> Vec<[u8; 32]> {
        if let Ok(Some(raw_accounts)) = client.session().get(&b"free_accounts".to_vec()) {
            return match Decode::decode(&mut raw_accounts.as_slice()) {
                Ok(free_accounts) => free_accounts,
                Err(_) => vec![],
            };
        }
        vec![]
    }
}
