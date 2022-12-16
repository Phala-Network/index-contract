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

    use engine::{RuningTaskFetcher, Step};
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

    /// Task creation lifetime cycle:
    ///
    /// - Source: Created
    /// - Phat: Found
    /// - Phat: Upload
    /// - Khala: Created
    /// - Source: Claimed (not before 4 to ensure it will not be dropped)
    /// - Phat: Executing
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

            init_worker_alloc();

            Self {
                admin: Self::env().caller(),
                registry: None,
                worker_accounts,
                executor_account: pink_web3::keys::pink::KeyPair::derive_keypair(b"executor")
                    .private_key(),
            }
        }

        /// Search actived tasks from source chain and upload them to rollup storage after worker
        /// account allocated
        #[ink(message)]
        pub fn upload_task(&self, source_chain: String) -> Result<(), Error> {
            // TODO: Maybe recover cache through onchain storage if cache is empty
            // By this way, if a task hasn't been claimed in source chain, and also
            // hasn't been saved in rollup storage, we can treat it as un-uploaded,
            // It's fine we re upload it, redundancy issue should be handled by rollup
            // handler pallet. e.g. pallet-index

            let free_workers = free_worker();

            // Fetch actived task that completed initial confirmation from specific chain that belong to current worker,
            // and append them to runing tasks
            let onchain_actived_tasks = ActivedTaskFetcher::new(self.registry.chains.get(chain).unwrap(), worker).fetch_tasks()?;
            /// TODO: Compare free_workers length with onchain_actived_tasks length
            for (idx, task) in onchain_actived_tasks {
                task.status = TaskStatus::Initialized;
                // Allocate worker account for this task
                task.worker = AccountInfo::Account30::from(free_workers[idx]);
            });

            // Upload tasks through off-chain rollup
            // ... create client
            // TODO: Except the task data, we also need have a pending task list
            client.action(Action::Reply(UploadToChain { task: task.clone() }.encode()));
            // ... 
            if let Some(submittable) = maybe_submittable {
                let tx_id = submittable
                    .submit(&self.executor_account, 0)
                    .log_err("failed to submit rollup tx")
                    .or(Err(Error::FailedToSendTransaction))?;

                // Add the new task to local cache
                // Since we already have tx hash, it can be used to track transactioin result
                // in `claim_task`, when it succeeds, `claim_task` start to claim task from
                // source chain
                // TODO: calculate nonce of this transaction
                task.status = TaskStatus::Uploading(Some(nonce));
                self.add_task(&task)?;
            }
        }

        /// Claim task from source chain when it was uploaded to rollup storage successfully
        #[ink(message)]
        pub fn claim_task(&self) -> Result<(), Error> {
            // TODO: Maybe recover cache through onchain storage if cache is empty
            // Here we need to query both rollup storage and source chain storage to determine
            // if the task should being uploaded.

            let mut task = self.get_task(&id).ok_or(Error::ExecuteFailed)?;
            match task.status {
                TaskStatus::Uploading(upload_tx_nonce) =>  {
                    let result = TransactionChecker::check_transaction(self.executor_account, task.chain, upload_tx_nonce)?;
                    // If claimed successfully, allocate worker account and upload it to pallet-index
                    if result.is_ok() {
                        // Claim task from source chain
                        let claim_tx_nonce = TaskClaimer::claim_task(&task.chain, &task.id)?;
                        task.status = TaskStatus::Claiming(Some(claim_tx_nonce));
                        self.update_task(&task);
                    }
                },
                _ => {
                    //
                }
            }
        }

        /// Execute tasks from all supported blockchains. This is a query operation
        /// that scheduler invokes periodically.
        #[ink(message)]
        pub fn execute_task(&self) -> Result<(), Error> {
            // Get the worker key that the contract deployed on
            let worker = pink_extension::ext().worker_pubkey();
            let mut running_tasks = self.running_tasks()?;

            // Recover cache if `running_tasks` is empty
            if running_tasks.len == 0 {
                let onchain_tasks = RuningTaskFetcher::new(&Chain(b"phala").endpoint, &worker).fetch_tasks()?;
                running_tasks = self.recover_cache(&onchain_tasks)?;
            }

            running_tasks.iter().map(|id| {
                let mut task = self.get_task(&id).ok_or(Error::ExecuteFailed)?;
                match task.status {
                    TaskStatus::Initialized =>  {
                        // Claim task from source chain
                        let hash = TaskClaimer::claim_task(&task.chain, &task.id)?;
                        task.status = TaskStatus::Claiming(Some(hash));
                        self.update_task(&task);
                    },
                    TaskStatus::Claiming(Some(claim_tx_nonce)) => {
                        let result = TransactionChecker::check_transaction(self.executor_account, task.chain, claim_tx_nonce)?;
                        // If claimed successfully, allocate worker account and upload it to pallet-index
                        if result {
                            // Allocate worker account (account used to send trasnaction to blockchain) to each claimed task,
                            // and upload them to the pending task queue saved in pallet-index
                            let free_accounts = self.free_worker();
                            if (free_accounts.len == 0) retuen;
                            task.worker = PublicKey(free_accounts[0]);
                            // Apply nonce to each step
                            self.apply_worker(&task)?;
                            // Mark as allocated
                            self.allocate_worker(&free_accounts[0])?;

                            // TODO: Check validity before upload

                            // Upload task to on-chain storage
                            let hash = TaskUploader::upload_task(self.executor_account, &worker, &local_claimed_tasks[index].task)?;
                            task.status = TaskStatus::Uploading(Some(hash));
                            self.update_task(&task);
                        }
                    },
                    TaskStatus::Executing(step_index, Some(execute_tx_nonce)) => {
                        // TODO: result should contains more information
                        let result = TransactionChecker::check_transaction(Step(self.registry)::source_chain(&task.edges[step_index]), execute_tx_nonce)?;
                        if result.is_ok() {
                            // If all steps executed completed, set task status as Completed
                            if step_index == task.edges.len - 1 {
                                task.status = TaskStatus::Completed;
                                self.update_task(&task);
                            } else {
                                // TODO: More validations

                                // Start to execute next step
                                let signer = PrivateKey(task.worker);
                                let hash = Step(self.registry)::execute_step(&signer, &task.edges[step_index + 1])?;
                                task.status = TaskStatus::Executing(step_index + 1, Some(hash));
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
                        self.remove_task(&task)?;

                        // TODO: update pallet-index
                    },
                    _ => {
                        // Do nothing
                    }
                }
            });

            Ok(())
        }

        fn apply_worker(&self, &mut task: Task, worker: [u8; 32]) -> Result<(), Error()> {
            // Parse chains that the task will interact with

            // Fetch worker nonce from each chain

            // Apply nonce to each step, same chain need increase nonce automatically
            // task.edges[index].nonce = Some(123);
        }

        fn running_tasks(&self) -> Result<Vec<TaskId>, Error> {
            pink_extension::ext().cache_get(b"running_tasks").ok_or(Err(ExecuteFailed))?;
        }

        fn add_task(&self, task: &Task) -> Result<(), Error> {
            let mut local_tasks = pink_extension::ext().cache_get(b"running_tasks").ok_or(Err(ExecuteFailed))?;
            if !local_tasks.contains(&task.id) {
                local_tasks.push(&task.id);
                pink_extension::ext().cache_set(b"running_tasks", &local_tasks)?;
                // Save full task
                pink_extension::ext().cache_set(&task.id, task)?;
            }
            Ok(())
        }

        fn remove_task(&self, task: &Task) -> Result<(), Error> {
            let mut local_tasks = pink_extension::ext().cache_get(b"running_tasks").ok_or(Err(ExecuteFailed))?;
            if local_tasks.contains(&task.id) {
                local_tasks.remove(&task.id);
                pink_extension::ext().cache_set(b"running_tasks", &local_tasks)?;
                // Delete task record from cache
                pink_extension::ext().cache_remove(&cache_remove).ok_or(Error::ExecuteFailed)?;
            }
            OK(())
        }

        fn update_task(&self, task: &Task) -> Result<(), Error> {
            if let Some(_) = pink_extension::ext().cache_get(&task.id) {
                // Update task record
                pink_extension::ext().cache_set(&task.id, task)?;
            }
            OK(())
        }

        fn get_task(&self, id: &TaskId) -> Option<Task> {
            pink_extension::ext().cache_get(&id)
        }

        /// Mark an account as allocated, e.g. put it into local cache `alloc` queue.
        fn allocate_worker(&self, key: &[u8; 32]) -> Result<(), Error> {
            let alloc_list = pink_extension::ext().cache_get(b"alloc").ok_or(Err(ExecuteFailed))?;
            alloc_list.push(key);
            pink_extension::ext().cache_set(b"alloc", &alloc_list)?;
        }

        /// Retuen accounts that hasn't been allocated to a specific task
        fn free_worker(&self) -> Vec<[u8; 32]> {
            let free_list = vec![];
            let alloc_list = pink_extension::ext().cache_get(b"alloc").ok_or(Err(ExecuteFailed))?;
            sefl.worker_accounts.iter().map(|worker| {
                if !alloc_list.contains(&worker) {
                    free_list.push(worker);
                }
            });
            free_list
        }
    }
}