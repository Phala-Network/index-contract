#![cfg_attr(not(any(feature = "std", test)), no_std, no_main)]

extern crate alloc;

mod account;
mod actions;
mod assets;
mod call;
mod chain;
mod constants;
mod context;
mod gov;
mod registry;
mod step;
mod storage;
mod task;
mod task_deposit;
mod task_fetcher;
mod traits;
mod tx;
mod utils;

#[allow(clippy::large_enum_variant)]
#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_executor {
    use crate::account::AccountInfo;
    use crate::chain::ChainType;
    use crate::context::Context;
    use crate::gov::WorkerGov;
    use crate::registry::Registry;
    use crate::storage::StorageClient;
    use crate::task::{Task, TaskId, TaskStatus};
    use crate::task_deposit::Solution;
    use crate::task_fetcher::ActivedTaskFetcher;
    use alloc::{string::String, vec, vec::Vec};
    use ink::storage::traits::StorageLayout;
    use ink_env::call::FromAccountId;
    use pink_extension::ResultExt;
    use scale::{Decode, Encode};
    use worker_key_store::KeyStoreRef;

    use pink_web3::ethabi::Address;

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        BadOrigin,
        NotConfigured,
        ChainNotFound,
        ImportWorkerFailed,
        WorkerNotFound,
        FailedToReadStorage,
        FailedToSetupStorage,
        FailedToSendTransaction,
        FailedToFetchTask,
        FailedToInitTask,
        FailedToDestoryTask,
        FailedToUploadTask,
        FailedToUploadSolution,
        SolutionAlreadyExist,
        FailedToReApplyNonce,
        FailedToReRunTask,
        TaskNotFoundInStorage,
        UnexpectedChainType,
        ExecutorPaused,
        ExecutorNotPaused,
    }

    type Result<T> = core::result::Result<T, Error>;

    #[derive(Clone, Encode, Decode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Config {
        /// The storage provider url
        storage_url: String,
        /// Secret key of storage provider
        storage_key: String,
    }

    /// Event emitted when executor is configured.
    #[ink(event)]
    pub struct Configured;

    /// Event emitted when worker account is set to storage.
    #[ink(event)]
    pub struct WorkerSetToStorage;

    #[derive(Clone, Encode, Decode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum RunningType {
        // [source_chain, worker_sr25519_pub_key]
        Fetch(String, [u8; 32]),
        Execute,
    }

    #[ink(storage)]
    pub struct Executor {
        pub admin: AccountId,
        pub config: Option<Config>,
        pub registry: Registry,
        pub worker_prv_keys: Vec<[u8; 32]>,
        pub worker_accounts: Vec<AccountInfo>,
        pub is_paused: bool,
    }

    impl Default for Executor {
        fn default() -> Self {
            Self::default()
        }
    }

    impl Executor {
        #[ink(constructor)]
        /// Create an Executor entity
        pub fn default() -> Self {
            Self {
                admin: Self::env().caller(),
                config: None,
                registry: Registry::new(),
                worker_prv_keys: vec![],
                worker_accounts: vec![],
                // Make sure we configured the executor before running
                is_paused: true,
            }
        }

        #[ink(message)]
        pub fn transfer_ownership(&mut self, new_admin: AccountId) -> Result<()> {
            self.ensure_owner()?;
            self.admin = new_admin;
            Ok(())
        }

        /// Debug only, remove before release
        #[ink(message)]
        pub fn export_worker_keys(&self) -> Result<Vec<[u8; 32]>> {
            self.ensure_owner()?;
            Ok(self.worker_prv_keys.clone())
        }

        /// FIXME: Pass the key implicitly
        #[ink(message)]
        pub fn config_engine(
            &mut self,
            storage_url: String,
            storage_key: String,
            keystore_account: AccountId,
            import_key: bool,
        ) -> Result<()> {
            self.ensure_owner()?;
            self.config = Some(Config {
                storage_url,
                storage_key,
            });

            // Import worker private key form keystore contract, make sure executor already set in keystore contract
            if import_key {
                let key_store_contract = KeyStoreRef::from_account_id(keystore_account);
                self.worker_prv_keys = key_store_contract
                    .get_worker_keys()
                    .or(Err(Error::ImportWorkerFailed))?;
                for key in self.worker_prv_keys.iter() {
                    self.worker_accounts.push(AccountInfo::from(*key))
                }
            }
            pink_extension::debug!(
                "Configured information as: {:?}, imported worker accounts: {:?}",
                &self.config,
                self.worker_accounts.clone()
            );
            Self::env().emit_event(Configured {});
            Ok(())
        }

        /// Save worker account information to remote storage.
        #[ink(message)]
        pub fn config_storage(&self) -> Result<()> {
            self.ensure_owner()?;

            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());

            pink_extension::debug!("Start to config storage");
            let accounts: Vec<[u8; 32]> = self
                .worker_accounts
                .clone()
                .into_iter()
                .map(|account| account.account32)
                .collect();
            client
                .alloc_storage(b"free_accounts", &accounts.encode())
                .map_err(|_| Error::FailedToSetupStorage)?;

            let empty_tasks: Vec<TaskId> = vec![];
            client
                .alloc_storage(b"pending_tasks", &empty_tasks.encode())
                .map_err(|_| Error::FailedToSetupStorage)?;
            Self::env().emit_event(WorkerSetToStorage {});
            Ok(())
        }

        #[ink(message)]
        pub fn update_registry(
            &mut self,
            chain: String,
            endpoint: String,
            indexer: String,
        ) -> Result<()> {
            self.ensure_owner()?;

            if let Some(index) = self.registry.chains.iter().position(|x| x.name == chain) {
                // Update the value at the found index
                self.registry.chains[index].endpoint = endpoint;
                self.registry.chains[index].tx_indexer = indexer;
            }
            Ok(())
        }

        #[ink(message)]
        pub fn pause_executor(&mut self) -> Result<()> {
            self.ensure_owner()?;
            self.ensure_running()?;
            self.is_paused = true;
            Ok(())
        }

        #[ink(message)]
        pub fn resume_executor(&mut self) -> Result<()> {
            self.ensure_owner()?;
            self.ensure_paused()?;
            self.is_paused = false;
            Ok(())
        }

        /// Submit transaction to execute ERC20 approve on behalf of the call on an EVM chain.
        #[ink(message)]
        pub fn worker_approve(
            &self,
            worker: [u8; 32],
            chain: String,
            token: [u8; 20],
            spender: [u8; 20],
            amount: u128,
        ) -> Result<()> {
            self.ensure_owner()?;
            let _ = self.ensure_configured()?;
            // To avoid race condiction happened on `nonce`, we should make sure no task will be executed.
            self.ensure_paused()?;

            let chain = self
                .registry
                .get_chain(&chain)
                .ok_or(Error::ChainNotFound)?;
            if chain.chain_type != ChainType::Evm {
                return Err(Error::UnexpectedChainType);
            }
            WorkerGov::erc20_approve(
                self.pub_to_prv(worker).ok_or(Error::WorkerNotFound)?,
                chain.endpoint,
                token.into(),
                spender.into(),
                amount,
            )
            .log_err("failed to submit worker approve tx")
            .or(Err(Error::FailedToSendTransaction))?;
            Ok(())
        }

        #[ink(message)]
        pub fn worker_drop_task(&self, worker: [u8; 32], chain: String, id: TaskId) -> Result<()> {
            self.ensure_owner()?;
            let _ = self.ensure_configured()?;

            // To avoid race condiction happened on `nonce`, we should make sure no task will be executed.
            self.ensure_paused()?;

            let chain = self
                .registry
                .get_chain(&chain)
                .ok_or(Error::ChainNotFound)?;

            if chain.chain_type != ChainType::Evm {
                return Err(Error::UnexpectedChainType);
            }
            WorkerGov::drop_task(
                self.pub_to_prv(worker).ok_or(Error::WorkerNotFound)?,
                chain.endpoint,
                Address::from_slice(&chain.handler_contract),
                id,
            )
            .log_err("failed to submit worker drop task tx")
            .or(Err(Error::FailedToSendTransaction))?;

            Ok(())
        }

        #[ink(message)]
        pub fn upload_solution(&self, id: TaskId, solution: Vec<u8>) -> Result<()> {
            self.ensure_running()?;
            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());

            let solution_id = [b"solution".to_vec(), id.to_vec()].concat();
            if client
                .read_storage::<Vec<u8>>(&solution_id)
                .map_err(|_| Error::FailedToReadStorage)?
                .is_some()
            {
                return Err(Error::SolutionAlreadyExist);
            }

            client
                .alloc_storage(&solution_id, &solution)
                .log_err("failed to upload solution")
                .or(Err(Error::FailedToUploadSolution))?;

            Ok(())
        }

        #[ink(message)]
        pub fn run(&self, running_type: RunningType) -> Result<()> {
            self.ensure_running()?;

            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());

            match running_type {
                RunningType::Fetch(source_chain, worker) => {
                    self.fetch_task(&client, &source_chain, worker)?
                }
                RunningType::Execute => self.execute_task(&client)?,
            };

            Ok(())
        }

        /// Re-apply nonce for task steps from current execution index, then re-run the failed step
        ///
        /// This is used to retry task when it failed to execute in current step indicated by `execute_index`
        #[ink(message)]
        pub fn retry(&self, id: TaskId) -> Result<()> {
            self.ensure_running()?;
            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());
            let (mut task, task_doc) = client
                .read_storage::<Task>(&id)
                .map_err(|_| Error::FailedToReadStorage)?
                .ok_or(Error::TaskNotFoundInStorage)?;

            if let TaskStatus::Executing(execute_index, _) = task.status {
                let context = Context {
                    signer: self.pub_to_prv(task.worker).unwrap(),
                    worker_accounts: self.worker_accounts.clone(),
                    registry: &self.registry,
                };
                task.reapply_nonce(execute_index as u64, &context, &client)
                    .map_err(|_| Error::FailedToReApplyNonce)?;
                pink_extension::info!(
                    "Step nonce re-applied from execution index: {:?}",
                    &execute_index
                );
                // Now re-run the step
                let _ = task
                    .execute_step(&context, &client)
                    .map_err(|_| Error::FailedToReRunTask)?;
                // Upload task data to storage
                client
                    .update_storage(task.id.as_ref(), &task.encode(), task_doc)
                    .map_err(|_| Error::FailedToUploadTask)?;
            }
            Ok(())
        }

        /// Return config information
        #[ink(message)]
        pub fn get_config(&self) -> Result<Option<Config>> {
            Ok(self.config.clone())
        }

        /// Return executor status
        #[ink(message)]
        pub fn is_running(&self) -> Result<bool> {
            Ok(!self.is_paused)
        }

        #[ink(message)]
        pub fn get_all_running_tasks(&self) -> Result<Vec<Task>> {
            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());

            let mut tasks = Vec::new();
            if let Some((ids, _)) = client
                .read_storage::<Vec<[u8; 32]>>(b"pending_tasks")
                .map_err(|_| Error::FailedToReadStorage)?
            {
                for id in ids.iter() {
                    pink_extension::debug!(
                        "Trying to read pending task data from remote storage, task id: {:?}",
                        &hex::encode(id)
                    );
                    let (task, _) = client
                        .read_storage::<Task>(id)
                        .map_err(|_| Error::FailedToReadStorage)?
                        .ok_or(Error::TaskNotFoundInStorage)?;

                    tasks.push(task);
                }
            }
            Ok(tasks)
        }

        #[ink(message)]
        pub fn get_running_task(&self, id: TaskId) -> Result<Option<Task>> {
            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());
            Ok(client
                .read_storage::<Task>(&id)
                .map_err(|_| Error::FailedToReadStorage)?
                .map(|(task, _)| task))
        }

        #[ink(message)]
        pub fn get_solution(&self, id: TaskId) -> Result<Option<Vec<u8>>> {
            self.ensure_running()?;
            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());

            let solution_id = [b"solution".to_vec(), id.to_vec()].concat();
            Ok(client
                .read_storage::<Solution>(&solution_id)
                .map_err(|_| Error::FailedToReadStorage)?
                .map(|(solution, _)| solution.encode()))
        }

        /// Returs the interior registry, callable to all
        #[ink(message)]
        pub fn get_registry(&self) -> Result<Registry> {
            Ok(self.registry.clone())
        }

        /// Return whole worker account information
        #[ink(message)]
        pub fn get_worker_accounts(&self) -> Result<Vec<AccountInfo>> {
            Ok(self.worker_accounts.clone())
        }

        /// Return worker accounts information that is free
        #[ink(message)]
        pub fn get_free_worker_account(&self) -> Result<Vec<[u8; 32]>> {
            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());
            if let Some((accounts, _)) = client
                .read_storage::<Vec<[u8; 32]>>(b"free_accounts")
                .map_err(|_| Error::FailedToReadStorage)?
            {
                Ok(accounts)
            } else {
                Ok(vec![])
            }
        }

        /// Search actived tasks from source chain and upload them to storage
        pub fn fetch_task(
            &self,
            client: &StorageClient,
            source_chain: &String,
            // Worker sr25519 public key
            worker: [u8; 32],
        ) -> Result<()> {
            let signer = self.pub_to_prv(worker).ok_or(Error::WorkerNotFound)?;
            // Fetch one actived task that completed initial confirmation from specific chain that belong to current worker
            let actived_task = ActivedTaskFetcher::new(
                self.registry
                    .get_chain(source_chain)
                    .ok_or(Error::ChainNotFound)?,
                AccountInfo::from(signer),
            )
            .fetch_task(client)
            .map_err(|_| Error::FailedToFetchTask)?;
            let Some(mut actived_task) = actived_task else {
                pink_extension::debug!("No actived task found from {:?}", &source_chain);
                return Ok(())
            };

            if client
                .read_storage::<Task>(&actived_task.id)
                .map_err(|_| Error::FailedToReadStorage)?
                .is_some()
            {
                pink_extension::debug!(
                    "Task {:?} already initialized, return",
                    hex::encode(actived_task.id)
                );
                return Ok(());
            }
            // Initialize task, and save it to on-chain storage
            actived_task
                .init(
                    &Context {
                        signer,
                        registry: &self.registry,
                        worker_accounts: self.worker_accounts.clone(),
                    },
                    client,
                )
                .map_err(|e| {
                    pink_extension::info!(
                        "Initial error {:?}, initialized task data: {:?}",
                        &e,
                        &actived_task
                    );
                    Error::FailedToInitTask
                })?;
            pink_extension::info!(
                "An actived task was found on {:?}, initialized task data: {:?}",
                &source_chain,
                &actived_task
            );

            Ok(())
        }

        /// Execute tasks from all supported blockchains. This is a query operation
        /// that scheduler invokes periodically.
        pub fn execute_task(&self, client: &StorageClient) -> Result<()> {
            if let Some((ids, _)) = client
                .read_storage::<Vec<[u8; 32]>>(b"pending_tasks")
                .map_err(|_| Error::FailedToReadStorage)?
            {
                for id in ids.iter() {
                    pink_extension::debug!(
                        "Trying to read pending task data from remote storage, task id: {:?}",
                        &hex::encode(id)
                    );
                    let (mut task, task_doc) = client
                        .read_storage::<Task>(id)
                        .map_err(|_| Error::FailedToReadStorage)?
                        .ok_or(Error::TaskNotFoundInStorage)?;

                    pink_extension::info!(
                        "Start execute task, execute worker account: {:?}",
                        &hex::encode(task.worker)
                    );
                    match task.execute(
                        &Context {
                            signer: self.pub_to_prv(task.worker).unwrap(),
                            worker_accounts: self.worker_accounts.clone(),
                            registry: &self.registry,
                        },
                        client,
                    ) {
                        Ok(TaskStatus::Completed) => {
                            pink_extension::info!(
                                "Task execution completed, delete it from storage: {:?}",
                                hex::encode(task.id)
                            );
                            // Remove task from blockchain and recycle worker account
                            task.destroy(client)
                                .map_err(|_| Error::FailedToDestoryTask)?;
                        }
                        Err(err) => {
                            pink_extension::error!(
                                "Failed to execute task on step {:?} with error {}, task data: {:?}",
                                task.execute_index,
                                err,
                                &task
                            );

                            // Execution failed, prepare necessary informations that DAO can handle later.
                            // Informatios should contains:
                            // 1. Sender on source chain
                            // 2. Current step
                            // 3. The allocated worker account
                            // 4. Current asset that worker account hold
                            //
                        }
                        _ => {
                            pink_extension::info!(
                                "Task execution has not finished yet, update data to remote storage: {:?}",
                                hex::encode(task.id)
                            );
                        }
                    }
                    client
                        .update_storage(task.id.as_ref(), &task.encode(), task_doc)
                        .map_err(|_| Error::FailedToUploadTask)?;
                }
            }

            Ok(())
        }

        /// Returns BadOrigin error if the caller is not the owner
        fn ensure_owner(&self) -> Result<()> {
            if self.env().caller() == self.admin {
                Ok(())
            } else {
                Err(Error::BadOrigin)
            }
        }

        /// Returns the config reference or raise the error `NotConfigured`
        fn ensure_configured(&self) -> Result<&Config> {
            self.config.as_ref().ok_or(Error::NotConfigured)
        }

        fn ensure_paused(&self) -> Result<()> {
            if !self.is_paused {
                return Err(Error::ExecutorNotPaused);
            }
            Ok(())
        }

        fn ensure_running(&self) -> Result<()> {
            if self.is_paused {
                return Err(Error::ExecutorPaused);
            }
            Ok(())
        }

        fn pub_to_prv(&self, pub_key: [u8; 32]) -> Option<[u8; 32]> {
            self.worker_accounts
                .iter()
                .position(|a| a.account32 == pub_key)
                .map(|idx| self.worker_prv_keys[idx])
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        // use dotenv::dotenv;
        // use pink_extension::PinkEnvironment;
        use xcm::v3::{prelude::*, MultiLocation};

        fn deploy_executor() -> Executor {
            // Deploy Executor
            Executor::default()
        }

        #[ignore]
        #[ink::test]
        fn storage_should_work() {
            pink_extension_runtime::mock_ext::mock_all_ext();
            let mut executor = deploy_executor();
            // Initial executor
            assert_eq!(
                executor.config_engine("url".to_string(), "key".to_string(), [0; 32].into(), true),
                Ok(())
            );
        }

        #[ink::test]
        fn dump_location() {
            println!(
                "Encode location: {:?}",
                hex::encode(
                    MultiLocation::new(
                        1,
                        X2(
                            Parachain(2000),
                            crate::utils::slice_to_generalkey(&hex_literal::hex!["0081"]),
                        )
                    )
                    .encode()
                )
            )
        }
    }
}
