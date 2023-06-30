#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

mod account;
mod context;
mod gov;
mod graph;
mod steps;
mod storage;
mod task;
mod traits;
mod tx;

#[allow(clippy::large_enum_variant)]
#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_executor {
    use crate::account::AccountInfo;
    use crate::context::Context;
    use crate::gov::WorkerGov;
    use crate::graph::Graph as RegistryGraph;
    use crate::steps::claimer::ActivedTaskFetcher;
    use crate::storage::StorageClient;
    use crate::task::{Task, TaskId, TaskStatus};
    use alloc::{boxed::Box, string::String, vec, vec::Vec};
    use index::prelude::*;
    use index::traits::executor::TransferExecutor;
    use index::utils::ToArray;
    use index::{
        graph::{Chain, ChainType, Graph},
        prelude::AcalaDexExecutor,
    };
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
        FailedToSendTransaction,
        FailedToFetchTask,
        FailedToInitTask,
        FailedToDestoryTask,
        ReadCacheFailed,
        WriteCacheFailed,
        DecodeCacheFailed,
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
        /// The URL of google firebase db
        db_url: String,
        /// The access token of google firebase db
        db_token: String,
    }

    /// Event emitted when graph is set.
    #[ink(event)]
    pub struct GraphSet;

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
        pub graph: Vec<u8>,
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
                graph: Vec::default(),
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
            db_url: String,
            db_token: String,
            keystore_account: AccountId,
        ) -> Result<()> {
            self.ensure_owner()?;
            // Insert empty record in advance
            let empty_tasks: Vec<TaskId> = vec![];
            pink_extension::ext()
                .cache_set(b"running_tasks", &empty_tasks.encode())
                .unwrap();
            self.config = Some(Config {
                db_url,
                db_token,
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

        /// Sets the graph, callable only to a specifically crafted management tool,
        /// should not be called by anyone else
        #[ink(message)]
        pub fn set_graph(&mut self, graph: RegistryGraph) -> Result<()> {
            // self.ensure_owner()?;
            self.graph = TryInto::<Graph>::try_into(graph)
                .or(Err(Error::SetGraphFailed))?
                .encode();
            Self::env().emit_event(GraphSet {});
            Ok(())
        }

        /// Save worker account information to remote storage.
        #[ink(message)]
        pub fn setup_worker_on_storage(&self) -> Result<()> {
            self.ensure_owner()?;

            let config = self.ensure_configured()?;
            let client = StorageClient::new(config.db_url.clone(), config.storage_key.clone());

            // Setup worker accounts if it hasn't been set yet.
            if client.lookup_free_accounts().is_none() {
                pink_extension::debug!("No onchain worker account exist, start setting to storage");
                client.set_worker_accounts(
                    self.worker_accounts
                        .clone()
                        .into_iter()
                        .map(|account| account.account32)
                        .collect(),
                );
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
            let client = StorageClient::new(config.db_url.clone(), config.db_token.clone());

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

        /// Returs the interior graph, callable to all
        #[ink(message)]
        pub fn get_graph(&self) -> Result<RegistryGraph> {
            let graph: Graph =
                Decode::decode(&mut self.graph.as_slice()).map_err(|_| Error::DecodeGraphFailed)?;
            Ok(graph.into())
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
            let client = StorageClient::new(config.db_url.clone(), config.db_token.clone());

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
                            graph: {
                                let bytes = self.graph.clone();
                                let mut bytes = bytes.as_ref();
                                Graph::decode(&mut bytes).unwrap()
                            },
                            worker_accounts: self.worker_accounts.clone(),
                            bridge_executors: vec![],
                            dex_executors: vec![],
                            transfer_executors: vec![],
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
            let bridge_executors = self.create_bridge_executors()?;
            let dex_executors = self.create_dex_executors()?;
            let transfer_executors = self.create_transfer_executors()?;

            for id in client.lookup_pending_tasks().iter() {
                pink_extension::debug!(
                    "Found one pending tasks exist in storge, task id: {:?}",
                    &hex::encode(id)
                );
                // Get task saved in local cache, if not exist in local, try recover from on-chain storage
                // FIXME: First time execute the task, it would be treat as broken, then trying to recover
                let mut task = TaskCache::get_task(id)
                    .or_else(|| {
                        pink_extension::warn!("Task data lost in local cache unexpectedly, try recover from storage, task id: {:?}", &hex::encode(id));
                        if let Some(mut onchain_task) = client.lookup_task(id) {
                            // The state of task saved in storage is `Initialized`, to understand
                            // the current state we must sync state according to on-chain history
                            onchain_task.sync(
                                &Context {
                                    signer: self.pub_to_prv(onchain_task.worker).unwrap(),
                                    graph: {
                                        let bytes = self.graph.clone();
                                        let mut bytes = bytes.as_ref();
                                        Graph::decode(&mut bytes).unwrap()
                                    },
                                    worker_accounts: self.worker_accounts.clone(),
                                    bridge_executors: vec![],
                                    dex_executors: vec![],
                                    transfer_executors: vec![],
                                },
                                client,
                            );
                            // Add task to local cache
                            let _ = TaskCache::add_task(&onchain_task);
                            pink_extension::info!("Task has been recovered successfully, recovered task data: {:?}", &onchain_task);
                            Some(onchain_task)
                        } else {
                            None
                        }
                    })
                    .ok_or(Error::TaskNotFoundOnChain)?;

                pink_extension::info!(
                    "Start execute next step of task, execute worker account: {:?}",
                    &hex::encode(task.worker)
                );
                match task.execute(
                    &Context {
                        signer: self.pub_to_prv(task.worker).unwrap(),
                        graph: {
                            let bytes = self.graph.clone();
                            let mut bytes = bytes.as_ref();
                            Graph::decode(&mut bytes).unwrap()
                        },
                        worker_accounts: self.worker_accounts.clone(),
                        bridge_executors: bridge_executors.clone(),
                        dex_executors: dex_executors.clone(),
                        transfer_executors: transfer_executors.clone(),
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

                        // If task already delete from storage, delete it from local cache
                        if client.lookup_task(id).is_none() {
                            pink_extension::info!(
                                "Task delete from storage, remove it from local cache: {:?}",
                                hex::encode(task.id)
                            );
                            TaskCache::remove_task(&task).map_err(|_| Error::WriteCacheFailed)?;
                        }
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
            let graph = Graph::decode(&mut &self.graph[..]).unwrap();
            graph.get_chain(name)
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
            if self.graph == Vec::<u8>::default() {
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

        #[allow(clippy::type_complexity)]
        fn create_bridge_executors(
            &self,
        ) -> Result<Vec<((String, String), Box<dyn BridgeExecutor>)>> {
            let mut bridge_executors: Vec<((String, String), Box<dyn BridgeExecutor>)> = vec![];
            let moonbeam = self
                .get_chain(String::from("Moonbeam"))
                .ok_or(Error::ChainNotFound)?;
            let phala = self
                .get_chain(String::from("Phala"))
                .ok_or(Error::ChainNotFound)?;
            let khala = self
                .get_chain(String::from("Khala"))
                .ok_or(Error::ChainNotFound)?;
            let ethereum = self
                .get_chain(String::from("Ethereum"))
                .ok_or(Error::ChainNotFound)?;

            let moonbeam_xtoken: [u8; 20] =
                hex_literal::hex!("0000000000000000000000000000000000000804");
            let chainbridge_on_ethereum: [u8; 20] =
                hex_literal::hex!("8F92e7353b180937895E0C5937d616E8ea1A2Bb9");

            // Moonbeam -> Acala
            bridge_executors.push((
                (String::from("Moonbeam"), String::from("Acala")),
                Box::new(MoonbeamXTokenExecutor::new(
                    &moonbeam.endpoint,
                    moonbeam_xtoken.into(),
                    ACALA_PARACHAIN_ID,
                )),
            ));
            // Moonbeam -> Phala
            bridge_executors.push((
                (String::from("Moonbeam"), String::from("Phala")),
                Box::new(MoonbeamXTokenExecutor::new(
                    &moonbeam.endpoint,
                    moonbeam_xtoken.into(),
                    PHALA_PARACHAIN_ID,
                )),
            ));
            // Phala -> Acala
            bridge_executors.push((
                (String::from("Phala"), String::from("Acala")),
                Box::new(PhalaXTransferExecutor::new(
                    &phala.endpoint,
                    ACALA_PARACHAIN_ID,
                    index::AccountType::Account32,
                )),
            ));
            // Ethereum -> Phala
            bridge_executors.push((
                (String::from("Ethereum"), String::from("Phala")),
                Box::new(ChainBridgeEthereum2Phala::new(
                    &ethereum.endpoint,
                    CHAINBRIDGE_ID_PHALA,
                    chainbridge_on_ethereum.into(),
                    vec![(
                        // PHA contract address on Ethereum
                        hex_literal::hex!("6c5bA91642F10282b576d91922Ae6448C9d52f4E").into(),
                        // PHA ChainBridge resource id on Phala
                        hex_literal::hex!(
                            "00b14e071ddad0b12be5aca6dffc5f2584ea158d9b0ce73e1437115e97a32a3e"
                        ),
                    )],
                )),
            ));
            // Phala -> Ethereum
            bridge_executors.push((
                (String::from("Phala"), String::from("Ethereum")),
                Box::new(ChainBridgePhala2Ethereum::new(
                    CHAINBRIDGE_ID_ETHEREUM,
                    &phala.endpoint,
                )),
            ));
            // Ethereum -> Khala
            bridge_executors.push((
                (String::from("Ethereum"), String::from("Khala")),
                Box::new(ChainBridgeEthereum2Phala::new(
                    &ethereum.endpoint,
                    CHAINBRIDGE_ID_KHALA,
                    chainbridge_on_ethereum.into(),
                    vec![(
                        // PHA contract address on Ethereum
                        hex_literal::hex!("6c5bA91642F10282b576d91922Ae6448C9d52f4E").into(),
                        // PHA ChainBridge resource id on Khala
                        hex_literal::hex!(
                            "00e6dfb61a2fb903df487c401663825643bb825d41695e63df8af6162ab145a6"
                        ),
                    )],
                )),
            ));
            // Khala -> Ethereum
            bridge_executors.push((
                (String::from("Khala"), String::from("Ethereum")),
                Box::new(ChainBridgePhala2Ethereum::new(
                    CHAINBRIDGE_ID_ETHEREUM,
                    &khala.endpoint,
                )),
            ));
            Ok(bridge_executors)
        }

        fn create_dex_executors(&self) -> Result<Vec<(String, Box<dyn DexExecutor>)>> {
            let mut dex_executors: Vec<(String, Box<dyn DexExecutor>)> = vec![];
            let moonbeam = self
                .get_chain(String::from("Moonbeam"))
                .ok_or(Error::ChainNotFound)?;
            let acala = self
                .get_chain(String::from("Acala"))
                .ok_or(Error::ChainNotFound)?;

            let stellaswap_router: [u8; 20] =
                hex::decode("70085a09D30D6f8C4ecF6eE10120d1847383BB57")
                    .unwrap()
                    .to_array();

            // Acala DEX
            dex_executors.push((
                String::from("Acala"),
                Box::new(AcalaDexExecutor::new(&acala.endpoint)),
            ));
            // Moonbeam::StellaSwap
            dex_executors.push((
                String::from("Moonbeam"),
                Box::new(MoonbeamDexExecutor::new(
                    &moonbeam.endpoint,
                    stellaswap_router.into(),
                )),
            ));
            Ok(dex_executors)
        }

        fn create_transfer_executors(&self) -> Result<Vec<(String, Box<dyn TransferExecutor>)>> {
            let mut transfer_executors: Vec<(String, Box<dyn TransferExecutor>)> = vec![];
            let acala = self
                .get_chain(String::from("Acala"))
                .ok_or(Error::ChainNotFound)?;
            transfer_executors.push((
                String::from("Acala"),
                Box::new(AcalaTransferExecutor::new(&acala.endpoint)),
            ));
            Ok(transfer_executors)
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

        #[ignore]
        #[ink::test]
        fn setup_worker_on_storage_should_work() {
            pink_extension_runtime::mock_ext::mock_all_ext();
            use crate::graph::Asset as RegistryAsset;
            use crate::graph::Chain as RegistryChain;
            let mut executor = deploy_executor();
            executor
                .set_graph(RegistryGraph {
                    chains: vec![RegistryChain {
                        id: 1,
                        name: "Khala".to_string(),
                        chain_type: 2,
                        endpoint: "http://127.0.0.1:39933".to_string(),
                        native_asset: 1,
                        foreign_asset_type: 1,
                        handler_contract: String::default(),
                        tx_indexer_url: Default::default(),
                    }],
                    assets: vec![RegistryAsset {
                        id: 1,
                        chain_id: 2,
                        name: "Phala Token".to_string(),
                        symbol: "PHA".to_string(),
                        decimals: 12,
                        location: hex::encode("Somewhere on Phala"),
                    }],
                    dexs: vec![],
                    bridges: vec![],
                    dex_pairs: vec![],
                    bridge_pairs: vec![],
                    dex_indexers: vec![],
                })
                .unwrap();
            // Initial executor
            assert_eq!(
                executor.config("url".to_string(), "key".to_string(), [0; 32].into()),
                Ok(())
            );
            assert_eq!(executor.setup_worker_on_storage(), Ok(()));
            let onchain_free_accounts = executor.get_free_worker_account().unwrap().unwrap();
            let local_worker_accounts: Vec<[u8; 32]> = executor
                .worker_accounts
                .into_iter()
                .map(|account| account.account32.clone())
                .collect();
            assert_eq!(onchain_free_accounts, local_worker_accounts);
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
