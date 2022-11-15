#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use ink_lang as ink;

mod task;

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

    use task::{ActivedTaskFetcher, Step, TaskClaimer, TaskUploader};

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
        pub registry: RegistryRef,
        pub worker_accounts: Vec<[u8; 32]>,
    }

    impl Default for Executor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Executor {
        #[ink(constructor)]
        /// Create an Executor entity
        pub fn new(register: AccountId) -> Self {
            ink_lang::utils::initialize_contract(|this: &mut Self| {
                this.admin = Self::env().caller();
                this.registry = RegistryRef::from_account_id(register);
                for index in 0..10 {
                    // TODO: generate private key
                    this.worker_accounts.push(vec![0; 32])
                }
                pink_extension::ext().cache_set(b"alloc", vec![])?;
            })
        }

        /// Claim task from all supported blockchains. This is a query operation
        /// that scheduler invokes periodically.
        /// 
        /// 
        /// 1) Perform spcific operations for the runing tasks.
        /// 2) Fetch new actived tasks from supported chains and append them to the local runing tasks queue.
        /// 
        /// Note Only when task upload completed, we remove it from `claimed_tasks` queue.
        #[ink::message]
        pub fn execute(&self) -> Result<(), Error> {
            // Get the worker key that the contract deployed on
            let worker = pink_extension::ext().worker_pubkey();
            let running_tasks = self.running_tasks()?;
            // 1) Check and execute runing tasks.
            running_tasks.iter().map(|id| {
                let mut task = self.get_task().ok_or(Error::ExecuteFailed)?;
                match task.status {
                    TaskStatus::Initialized =>  {
                        // Claim task from source chain
                        let hash = TaskClaimer::claim_task(&task.chain, &task.id)?;
                        task.status = TaskStatus::Claiming(Some(hash));
                        self.update_task(&task);
                    },
                    TaskStatus::Claiming(Some(claim_tx_hash)) => {
                        let result = TransactionChecker::check_transaction(task.chain, claim_tx_hash)?;
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

                            // Upload task to on-chain storage
                            let hash = TaskUploader::upload_task(&worker, &local_claimed_tasks[index].task)?;
                            task.status = TaskStatus::Uploading(Some(hash));
                            self.update_task(&task);
                        }
                    },
                    TaskStatus::Uploading(Some(upload_tx_hash)), => {
                        let result = TransactionChecker::check_transaction(task.chain, upload_tx_hash)?;
                        if result {
                            // Start to execute first step
                            let hash = Step::execute_step(&task.edges[0])?;
                            task.status = TaskStatus::Executing(0, Some(hash));
                            self.update_task(&task);
                        }
                    },
                    TaskStatus::Executing(step_index, Some(execute_tx_hash)) => {
                        let result = TransactionChecker::check_transaction(Step::source_chain(&task.edges[step_index]), execute_tx_hash)?;
                        if result {
                            // If all steps executed completed, set task status as Completed
                            if step_index == task.edges.len - 1 {
                                task.status = TaskStatus::Completed;
                                self.update_task(&task);
                            } else {
                                // TODO: More validations

                                // Start to execute next step
                                let hash = Step::execute_step(&task.edges[step_index + 1])?;
                                task.status = TaskStatus::Executing(step_index + 1, Some(hash));
                                self.update_task(&task);
                            }
                        } else {
                            // Re-apply nonce
                            self.apply_worker(&task)?;
                            // TODO: Retry

                            // TODO: Exceed retry limit, revert
                            let hash = Step::revert_step(&task.edges[step_index])?;
                            task.status = TaskStatus::Reverting(step_index, Some(hash));
                            self.update_task(&task);
                        }
                    },
                    TaskStatus::Reverting(step_index, Some(revert_tx_hash)) => {
                        let result = TransactionChecker::check_transaction(Step::dest_chain(&task.edges[step_index]), revert_tx_hash)?;
                        if result {
                                // TODO: More validations

                                // Start to execute next step
                                let hash = Step::revert_step(&task.edges[step_index - 1])?;
                                task.status = TaskStatus::Reverting(step_index - 1, Some(hash));
                                self.update_task(&task);
                        } eles {
                            // Re-apply nonce
                            self.apply_worker(&task)?;
                            // TODO: Retry

                            // TODO: Exceed retry limit, task is dead, manual handling???
                        }
                    },
                    TaskStatus::Completed => {
                        self.remove_task(&task)?;
                    }
                }
            });

            // 2) Fetch actived task that completed initial confirmation from all supported chains that belong to current worker,
            // and append them to runing tasks
            let onchain_actived_tasks = self.registry.supported_chains.iter().map(|chain| {
                let chain_info = self.registry.chains.get(chain).unwrap();
                let actived_tasks = ActivedTaskFetcher::new(chain_info, worker).fetch_tasks()?;
                actived_tasks
            }).collect().concat();
            /// Save to local cache
            onchain_actived_tasks.iter().map(|mut task| {
                task.status = TaskStatus::Initialized;
                self.add_task(&task);
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