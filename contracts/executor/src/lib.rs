#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use ink_lang as ink;

mod account;
mod engine;
mod worker;
mod tasks;
mod types;
mod uploader;
mod claimer;

#[allow(clippy::large_enum_variant)]
#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_executor {
    use alloc::vec::Vec;
    use index::ensure;
    use index::prelude::*;
    use index::prelude::{ChainInfo, Graph};
    use index::registry::bridge::{AssetPair, Bridge};
    use index::registry::chain::Chain;
    use index::registry::dex::{Dex, DexPair};
    use ink_storage::traits::SpreadAllocate;
    use ink_storage::Mapping;
    // use pallet_index::{};
    use ink_env::call::FromAccountId;
    use index_registry::{AccountInfo, AccountStatus, RegistryRef};

    use engine::{RuningTaskFetcher, StepExecutor, ExecutionChecker};
    use crate::types::{Task, TaskId, TaskStatus};
    use crate::account::AccountInfo;
    use crate::claimer::{ActivedTaskFetcher, TaskClaimer};
    use crate::uploader::{UploadToChain, TaskUploader};
    use crate::worker::*;
    use crate::task::*;

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        ExecuteFailed,
    }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Executor {
        pub admin: AccountId,
        pub registry: Option<RegistryRef>,
        pub worker_accounts: Vec<[u8; 32]>,
        pub executor_account: [u8; 32],
    }

    impl Default for Executor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Executor {
        #[ink(constructor)]
        /// Create an Executor entity
        pub fn new() -> Self {
            let mut worker_accounts: Vec<[u8; 32]> = vec![];
            for index in 0..10 {
                worker_accounts.push(
                    pink_web3::keys::pink::KeyPair::derive_keypair(
                        &[b"worker".to_vec(), [index].to_vec()].concat(),
                    )
                    .private_key(),
                )
            }

            Self {
                admin: Self::env().caller(),
                registry: None,
                worker_accounts,
                executor_account: pink_web3::keys::pink::KeyPair::derive_keypair(b"executor")
                    .private_key(),
            }
        }

        /// Search actived tasks from source chain and upload them to rollup storage
        #[ink(message)]
        pub fn fetch_task(&self, source_chain: String) -> Result<(), Error> {
            // Fetch actived task that completed initial confirmation from specific chain that belong to current worker,
            // and append them to runing tasks
            let mut onchain_actived_tasks = ActivedTaskFetcher::new(self.registry.chains.get(source_chain).unwrap(), self.executor_account).fetch_tasks()?;
            
            self.initialize_task_onchain(&onchain_actived_tasks);

            // Submit to blockchain
            if let Some(submittable) = maybe_submittable {
                let tx_id = submittable
                    .submit(&self.executor_account, 0)
                    .log_err("failed to submit rollup tx")
                    .or(Err(Error::FailedToSendTransaction))?;
            }
        }

        /// Execute tasks from all supported blockchains. This is a query operation
        /// that scheduler invokes periodically.
        #[ink(message)]
        pub fn execute_task(&self) -> Result<(), Error> {
            // Try recover cache from onchain storage if it is empty or crashed
            self.maybe_recover_cache()?;

            let local_tasks = get_all_task_local()?;

            for id in local_tasks.iter() {
                // Get task saved in local cache
                let mut task = self.get_task_local(&id).ok_or(Error::ExecuteFailed)?;
                let signer = self.pub_to_prv(&task.worker);

                match task.status {
                    TaskStatus::Initialized =>  {
                        // If task exist in local cache, that means it already been uploaded in rollup storage,
                        // next step, we claim it from source chain

                        // First step of execution is always to claim task from source chain
                        let nonce = StepExecutor(self.registry)::execute_step(&signer, &task.steps[0])?;
                        task.status = TaskStatus::Executing(0, Some(nonce));
                        self.update_task_local(&task);
                    },
                    TaskStatus::Executing(step_index, Some(execute_tx_nonce)) => {
                        // TODO: result should contains more information
                        let result = ExecutionChecker::check_execution(&task.steps[step_index], AccountInfo::from(signer))?;
                        if result.is_ok() {
                            // If all steps executed completed, set task status as Completed
                            if step_index == task.steps.len - 1 {
                                task.status = TaskStatus::Completed;
                                self.update_task(&task);
                            } else {
                                // Start to execute next step
                                let nonce = StepExecutor(self.registry)::execute_step(&signer, &task.steps[step_index + 1])?;
                                task.status = TaskStatus::Executing(step_index + 1, Some(nonce));
                                self.update_task(&task);
                            }
                        } else {
                            // Execution failed, prepare necessary informations that DAO can handle later.
                            // Informatios should contains:
                            // 1. Sender on source chain
                            // 2. Current step
                            // 3. The allocated worker account
                            // 4. Current asset that worker account hold
                            //
                        }
                    },
                    TaskStatus::Completed => {
                        // Remove task from blockchain and recycle worker account
                        self.destroy_task_onchain(&task);
                        // If task already delete from rollup storage, delete it from local cache
                        if self.lookup_task_onchain(task.id) == None {
                            remove_task_local(&task);
                        }
                    },
                }
            });

            Ok(())
        }

        fn initialize_task_onchain(&self, client: &SubstrateRollupClient, tasks: &mut Vec<Task>) {
            let client = RollupClient::new();
            let mut free_accounts: Vec<AccountInfo::Account32> = client.session.get(b"free_accounts".to_vec()).unwrap();
            let mut pending_tasks: Vec<TaskId> = client.session.get(b"pending_tasks".to_vec()).unwrap();

            for task in tasks.iter_mut() {
                if client.session.get(task.id).is_some() {
                    // Task already saved, skip
                    contine;
                }
                if let Some(account) = free_accounts.pop() {
                    // Apply a worker account
                    task.worker = account;
                    // Aplly worker nonce for each step in task
                    self.aplly_nonce(&task);
                    task.status = TaskStatus::Initialized;
                    // Push to pending tasks queue
                    pending_tasks.push(task.id);
                    // Save task data
                    client.session.put(task.id, task);
                } else {
                    // We can not handle more tasks any more
                    break;
                }
            }

            client.session.put(b"free_accounts".to_vec(), free_accounts);
            client.session.put(b"pending_tasks".to_vec(), pending_tasks);
            client.commit();
        }

        fn destroy_task_onchain(&self, client: &SubstrateRollupClient, tasks: &Task) {
            let client = RollupClient::new();
            let mut pending_tasks: Vec<TaskId> = client.session.get(b"pending_tasks".to_vec()).unwrap();
            let mut free_accounts: Vec<AccountInfo::Account32> = client.session.get(b"free_accounts".to_vec()).unwrap();

            if client.session.get(task.id).is_some() {
                if let Some(idx) = pending_tasks
                .iter()
                .position(|id| *id == task.id) {
                    // Remove from pending tasks queue
                    pending_tasks.remove(idx);
                    // Recycle worker account
                    free_accounts.push(task.worker);
                    // Delete task data
                    client.session.remove(task.id);
                }
                client.session.put(b"free_accounts".to_vec(), free_accounts);
                client.session.put(b"pending_tasks".to_vec(), pending_tasks);
                client.commit();
            }
        }

        fn lookup_task_onchain(&self, id: TaskId) -> Option<Task> {
            let client = RollupClient::new();
            client.session.get(id)
        }

        fn aplly_nonce(&self, task: &mut Task) -> {

        }

        fn maybe_recover_cache(&self) {

        }
}