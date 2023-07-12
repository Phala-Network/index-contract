#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

mod account;
mod chain;
mod context;
mod gov;
mod registry;
mod steps;
mod storage;
mod task;
mod traits;
mod tx;

#[allow(clippy::large_enum_variant)]
#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_executor {
    use crate::account::AccountInfo;
    use crate::chain::{Chain, ChainType};
    use crate::context::Context;
    use crate::gov::WorkerGov;
    use crate::registry::Registry;
    use crate::steps::claimer::ActivedTaskFetcher;
    use crate::storage::StorageClient;
    use crate::task::{Task, TaskId, TaskStatus};
    use alloc::{string::String, vec, vec::Vec};
    use ink::storage::traits::StorageLayout;
    use ink_env::call::FromAccountId;
    use pink_extension::ResultExt;
    use scale::{Decode, Encode};
    use worker_key_store::KeyStoreRef;

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        BadOrigin,
        NotConfigured,
        MissingPalletId,
        ChainNotFound,
        ImportWorkerFailed,
        WorkerNotFound,
        FailedToSetWorker,
        FailedToSendTransaction,
        FailedToFetchTask,
        FailedToInitTask,
        FailedToDestoryTask,
        FailedToUploadTask,
        DecodeGraphFailed,
        SetGraphFailed,
        TaskNotFoundInStorage,
        UnexpectedChainType,
        GraphNotSet,
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

        #[ink(message)]
        pub fn config(
            &mut self,
            storage_url: String,
            storage_key: String,
            keystore_account: AccountId,
        ) -> Result<()> {
            self.ensure_owner()?;
            self.config = Some(Config {
                storage_url,
                storage_key,
            });

            // Import worker private key form keystore contract, make sure executor already set in keystore contract
            let key_store_contract = KeyStoreRef::from_account_id(keystore_account);
            self.worker_prv_keys = key_store_contract
                .get_worker_keys()
                .or(Err(Error::ImportWorkerFailed))?;
            for key in self.worker_prv_keys.iter() {
                self.worker_accounts.push(AccountInfo::from(*key))
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
        pub fn setup_worker_on_storage(&self) -> Result<()> {
            self.ensure_owner()?;

            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());

            // Setup worker accounts if it hasn't been set yet.
            if client.lookup_free_accounts().is_none() {
                pink_extension::debug!("No onchain worker account exist, start setting to storage");
                client
                    .set_worker_accounts(
                        self.worker_accounts
                            .clone()
                            .into_iter()
                            .map(|account| account.account32)
                            .collect(),
                    )
                    .map_err(|_| Error::FailedToSetWorker)?;
            }
            Self::env().emit_event(WorkerSetToStorage {});
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
            self.ensure_graph_set()?;
            // To avoid race condiction happened on `nonce`, we should make sure no task will be executed.
            self.ensure_paused()?;

            let chain = self.get_chain(chain).ok_or(Error::ChainNotFound)?;
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
        pub fn run(&self, running_type: RunningType) -> Result<()> {
            self.ensure_running()?;
            self.ensure_graph_set()?;

            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());

            match running_type {
                RunningType::Fetch(source_chain, worker) => {
                    self.fetch_task(&client, source_chain, worker)?
                }
                RunningType::Execute => self.execute_task(&client)?,
            };

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
            // TODO: read from remote storage
            Ok(vec![])
        }

        #[ink(message)]
        pub fn get_running_task(&self, _task_id: TaskId) -> Result<Option<Task>> {
            // TODO: read from remote storage
            Ok(None)
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
        pub fn get_free_worker_account(&self) -> Result<Option<Vec<[u8; 32]>>> {
            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.storage_url.clone(), config.storage_key.clone());

            Ok(client.lookup_free_accounts())
        }

        /// Search actived tasks from source chain and upload them to storage
        pub fn fetch_task(
            &self,
            client: &StorageClient,
            source_chain: String,
            // Worker sr25519 public key
            worker: [u8; 32],
        ) -> Result<()> {
            // Fetch one actived task that completed initial confirmation from specific chain that belong to current worker
            let actived_task = ActivedTaskFetcher::new(
                self.get_chain(source_chain.clone())
                    .ok_or(Error::ChainNotFound)?,
                AccountInfo::from(self.pub_to_prv(worker).ok_or(Error::WorkerNotFound)?),
            )
            .fetch_task()
            .map_err(|_| Error::FailedToFetchTask)?;
            if let Some(mut actived_task) = actived_task {
                // Initialize task, and save it to on-chain storage
                actived_task
                    .init_and_submit(
                        &Context {
                            // Don't need signer here
                            signer: [0; 32],
                            registry: &self.registry,
                            worker_accounts: self.worker_accounts.clone(),
                        },
                        client,
                    )
                    .map_err(|_| Error::FailedToInitTask)?;
                pink_extension::info!(
                    "An actived task was found on {:?}, initialized task data: {:?}",
                    &source_chain,
                    &actived_task
                );
            } else {
                pink_extension::debug!("No actived task found from {:?}", &source_chain);
            }

            Ok(())
        }

        /// Execute tasks from all supported blockchains. This is a query operation
        /// that scheduler invokes periodically.
        pub fn execute_task(&self, client: &StorageClient) -> Result<()> {
            for id in client.lookup_pending_tasks().iter() {
                pink_extension::debug!(
                    "Found one pending tasks exist in storge, task id: {:?}",
                    &hex::encode(id)
                );
                pink_extension::debug!(
                    "Trying to read task data from remote storage, task id: {:?}",
                    &hex::encode(id)
                );
                let mut task: Task = client.lookup_task(id).ok_or(Error::TaskNotFoundInStorage)?;
                pink_extension::info!(
                    "Start execute next step of task, execute worker account: {:?}",
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
                    Err(_) => {
                        pink_extension::error!(
                            "Failed to execute task on step {:?}, task data: {:?}",
                            task.execute_index,
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
                        client
                            .put(task.id.as_ref(), &task.encode())
                            .map_err(|_| Error::FailedToUploadTask)?;
                        continue;
                    }
                }
            }

            Ok(())
        }

        pub fn get_chain(&self, name: String) -> Option<Chain> {
            self.registry.get_chain(name)
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

        fn ensure_graph_set(&self) -> Result<()> {
            if self.registry.chains.is_empty() {
                return Err(Error::GraphNotSet);
            }
            Ok(())
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
        use phala_pallet_common::WrapSlice;
        // use pink_extension::PinkEnvironment;
        use xcm::latest::{prelude::*, MultiLocation};

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
                executor.config("url".to_string(), "key".to_string(), [0; 32].into()),
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
                            GeneralKey(WrapSlice(&hex_literal::hex!["0081"]).into())
                        )
                    )
                    .encode()
                )
            )
        }
    }
}
