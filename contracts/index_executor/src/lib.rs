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
mod price;
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
    use crate::step::{MultiStep, Simulate as StepSimulate, StepSimulateResult};
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
        FailedToDecodeSolution,
        InvalidSolutionData,
        SolutionAlreadyExist,
        FailedToReApplyNonce,
        FailedToReRunTask,
        FailedToSimulateSolution,
        TaskNotFoundInStorage,
        UnexpectedChainType,
        ExecutorPaused,
        ExecutorNotPaused,
        MissingAssetInfo,
    }

    type Result<T> = core::result::Result<T, Error>;

    #[derive(Clone, Encode, Decode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Config {
        /// The URL of google firebase db
        db_url: String,
        /// The access token of google firebase db
        db_token: String,
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
        // [worker_sr25519_pub_key]
        Execute([u8; 32]),
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

        /// Debug only, remove before release
        #[ink(message)]
        pub fn import_worker_keys(&mut self, keys: Vec<[u8; 32]>) -> Result<()> {
            self.ensure_owner()?;
            self.worker_prv_keys = keys;
            for key in self.worker_prv_keys.iter() {
                self.worker_accounts.push(AccountInfo::from(*key))
            }
            Ok(())
        }

        /// FIXME: Pass the key implicitly
        #[ink(message)]
        pub fn config_engine(
            &mut self,
            db_url: String,
            db_token: String,
            keystore_account: AccountId,
            import_key: bool,
        ) -> Result<()> {
            self.ensure_owner()?;
            self.config = Some(Config { db_url, db_token });

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

        pub fn update_registry(
            &mut self,
            chain: String,
            endpoint: String,
            indexer_url: String,
        ) -> Result<()> {
            self.ensure_owner()?;

            if let Some(index) = self.registry.chains.iter().position(|x| x.name == chain) {
                // Update the value at the found index
                self.registry.chains[index].endpoint = endpoint;
                self.registry.chains[index].tx_indexer_url = indexer_url;
            }
            Ok(())
        }

        pub fn register_asset(
            &mut self,
            asset: crate::registry::Asset,
        ) -> Result<()> {
            self.ensure_owner()?;
            self.registry.assets.push(asset);
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
            let client = StorageClient::new(config.db_url.clone(), config.db_token.clone());

            let solution_id = [b"solution".to_vec(), id.to_vec()].concat();
            if client
                .read::<Solution>(&solution_id)
                .map_err(|_| Error::FailedToReadStorage)?
                .is_some()
            {
                return Err(Error::SolutionAlreadyExist);
            }

            client
                .insert(&solution_id, &solution)
                .log_err("failed to upload solution")
                .or(Err(Error::FailedToUploadSolution))?;

            Ok(())
        }

        #[ink(message)]
        pub fn simulate_solution(
            &self,
            worker: [u8; 32],
            solution: Vec<u8>,
        ) -> Result<Vec<StepSimulateResult>> {
            let solution: Solution =
                Decode::decode(&mut solution.as_slice()).or(Err(Error::FailedToDecodeSolution))?;

            let signer: [u8; 32] = self.pub_to_prv(worker).ok_or(Error::WorkerNotFound)?;
            let context = Context {
                signer,
                worker_accounts: self.worker_accounts.clone(),
                registry: &self.registry,
            };
            let mut simulate_results: Vec<StepSimulateResult> = vec![];
            for multi_step_input in solution.iter() {
                let mut multi_step: MultiStep = multi_step_input
                    .clone()
                    .try_into()
                    .or(Err(Error::InvalidSolutionData))?;
                let asset_location = multi_step.as_single_step().spend_asset;
                let asset_info = context
                    .registry
                    .get_asset(&multi_step.as_single_step().source_chain, &asset_location)
                    .ok_or(Error::MissingAssetInfo)?;
                // Set spend asset 0.0001
                multi_step.set_spend(1 * 10u128.pow(asset_info.decimals as u32) / 10000);
                let step_simulate_result = multi_step
                    .simulate(&Context {
                        signer,
                        registry: &self.registry,
                        worker_accounts: self.worker_accounts.clone(),
                    })
                    .map_err(|err| {
                        pink_extension::error!("Solution simulation failed with error: {}", err);
                        Error::FailedToSimulateSolution
                    })?;
                simulate_results.push(step_simulate_result);
            }

            Ok(simulate_results)
        }

        #[ink(message)]
        pub fn run(&self, running_type: RunningType) -> Result<()> {
            self.ensure_running()?;

            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.db_url.clone(), config.db_token.clone());

            match running_type {
                RunningType::Fetch(source_chain, worker) => {
                    self.fetch_task(&client, &source_chain, worker)?
                }
                RunningType::Execute(worker) => self.execute_task(&client, worker)?,
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
            let client = StorageClient::new(config.db_url.clone(), config.db_token.clone());
            let (mut task, task_doc) = client
                .read::<Task>(&id)
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
                    .update(task.id.as_ref(), &task.encode(), task_doc)
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
            let client = StorageClient::new(config.db_url.clone(), config.db_token.clone());

            let mut tasks: Vec<Task> = Vec::new();
            for worker_account in self.worker_accounts.iter() {
                if let Some((task_id, _)) = client
                    .read::<TaskId>(&worker_account.account32)
                    .map_err(|_| Error::FailedToReadStorage)?
                {
                    pink_extension::debug!(
                        "Trying to read pending task data from remote storage, task id: {:?}",
                        &hex::encode(task_id)
                    );
                    let (task, _) = client
                        .read::<Task>(&task_id)
                        .map_err(|_| Error::FailedToReadStorage)?
                        .ok_or(Error::TaskNotFoundInStorage)?;

                    tasks.push(task);
                }
            }

            Ok(tasks)
        }

        #[ink(message)]
        pub fn get_task(&self, id: TaskId) -> Result<Option<Task>> {
            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.db_url.clone(), config.db_token.clone());
            Ok(client
                .read::<Task>(&id)
                .map_err(|_| Error::FailedToReadStorage)?
                .map(|(task, _)| task))
        }

        #[ink(message)]
        pub fn get_solution(&self, id: TaskId) -> Result<Option<Vec<u8>>> {
            self.ensure_running()?;
            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.db_url.clone(), config.db_token.clone());

            let solution_id = [b"solution".to_vec(), id.to_vec()].concat();
            Ok(client
                .read::<Solution>(&solution_id)
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
        pub fn execute_task(&self, client: &StorageClient, worker: [u8; 32]) -> Result<()> {
            if let Some((id, _)) = client
                .read::<TaskId>(&worker)
                .map_err(|_| Error::FailedToReadStorage)?
            {
                pink_extension::debug!(
                    "Trying to read pending task data from remote storage, task id: {:?}",
                    &hex::encode(id)
                );
                let (mut task, task_doc) = client
                    .read::<Task>(&id)
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
                    .update(task.id.as_ref(), &task.encode(), task_doc)
                    .map_err(|_| Error::FailedToUploadTask)?;
            } else {
                pink_extension::debug!(
                    "No pending task to execute for worker: {:?}, return",
                    &hex::encode(worker)
                );
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
        use crate::step::{MultiStepInput, StepInput};
        // use dotenv::dotenv;
        // use pink_extension::PinkEnvironment;
        use crate::utils::ToArray;
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

        #[ink::test]
        #[ignore]
        fn simulate_solution_should_work() {
            pink_extension_runtime::mock_ext::mock_all_ext();
            let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
            let secret_key = secret_key.unwrap().1;
            let secret_bytes = hex::decode(secret_key).unwrap();
            let worker_key: [u8; 32] = secret_bytes.to_array();
            let mut executor = deploy_executor();
            assert_eq!(executor.import_worker_keys(vec![worker_key]), Ok(()));
            assert_eq!(
                executor.config_engine("url".to_string(), "key".to_string(), [0; 32].into(), false),
                Ok(())
            );
            assert_eq!(executor.resume_executor(), Ok(()));

            let solution1: Vec<MultiStepInput> = vec![
                MultiStepInput::Batch(vec![
                    StepInput {
                        exe: "ethereum_nativewrapper".to_string(),
                        source_chain: "Ethereum".to_string(),
                        dest_chain: "Ethereum".to_string(),
                        spend_asset: "0x0000000000000000000000000000000000000000".to_string(),
                        // WETH
                        receive_asset: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                        recipient: "0xd693bDC5cb0cF2a31F08744A0Ec135a68C26FE1c".to_string(),
                    },
                    StepInput {
                        exe: "ethereum_uniswapv2".to_string(),
                        source_chain: "Ethereum".to_string(),
                        dest_chain: "Ethereum".to_string(),
                        spend_asset: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                        // PHA
                        receive_asset: "0x6c5bA91642F10282b576d91922Ae6448C9d52f4E".to_string(),
                        recipient: "0xd693bDC5cb0cF2a31F08744A0Ec135a68C26FE1c".to_string(),
                    },
                    StepInput {
                        exe: "ethereum_sygmabridge_to_phala".to_string(),
                        source_chain: "Ethereum".to_string(),
                        dest_chain: "Phala".to_string(),
                        spend_asset: "0x6c5bA91642F10282b576d91922Ae6448C9d52f4E".to_string(),
                        // PHA
                        receive_asset: "0x0000".to_string(),
                        recipient:
                            "0x641017970d80738617e4e9b9b01d8d2ed5bc3d881a60e5105620abfbf5cb1331"
                                .to_string(),
                    },
                ]),
                MultiStepInput::Single(StepInput {
                    exe: "phala_bridge_to_astar".to_string(),
                    source_chain: "Phala".to_string(),
                    dest_chain: "Astar".to_string(),
                    spend_asset: "0x0000".to_string(),
                    // PHA
                    receive_asset: "0x010100cd1f".to_string(),
                    recipient: "0x641017970d80738617e4e9b9b01d8d2ed5bc3d881a60e5105620abfbf5cb1331"
                        .to_string(),
                }),
            ];

            let result1 = executor
                .simulate_solution(executor.worker_accounts[0].account32, solution1.encode())
                .unwrap();
            println!("simulation result1: {:?}", result1);

            let solution2: Vec<MultiStepInput> = vec![
                MultiStepInput::Single(StepInput {
                    exe: "khala_bridge_to_ethereum".to_string(),
                    source_chain: "Khala".to_string(),
                    dest_chain: "Ethereum".to_string(),
                    spend_asset: "0x0000".to_string(),
                    receive_asset: "0x6c5bA91642F10282b576d91922Ae6448C9d52f4E".to_string(),
                    recipient: "0x5cddb3ad187065e0122f3f46d13ad6ca486e4644".to_string(),
                }),
                MultiStepInput::Single(StepInput {
                    exe: "ethereum_sygmabridge_to_phala".to_string(),
                    source_chain: "Ethereum".to_string(),
                    dest_chain: "Phala".to_string(),
                    spend_asset: "0x6c5bA91642F10282b576d91922Ae6448C9d52f4E".to_string(),
                    // PHA
                    receive_asset: "0x0000".to_string(),
                    recipient: "0x641017970d80738617e4e9b9b01d8d2ed5bc3d881a60e5105620abfbf5cb1331"
                        .to_string(),
                }),
            ];
            let result2 = executor
                .simulate_solution(executor.worker_accounts[0].account32, solution2.encode())
                .unwrap();
            println!("simulation result2: {:?}", result2);
        }
    }
}
