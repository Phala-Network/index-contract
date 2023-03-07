#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;
use ink_lang as ink;

mod account;
mod bridge;
mod cache;
mod claimer;
mod context;
mod graph;
mod step;
mod swap;
mod task;
mod traits;

#[allow(clippy::large_enum_variant)]
#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_executor {
    use crate::account::AccountInfo;
    use crate::cache::*;
    use crate::claimer::ActivedTaskFetcher;
    use crate::context::Context;
    use crate::graph::Graph as RegistryGraph;
    use crate::task::{OnchainAccounts, OnchainTasks, Task, TaskId, TaskStatus};
    use alloc::{boxed::Box, string::String, vec, vec::Vec};
    use index::prelude::*;
    use index::utils::ToArray;
    use index::{
        graph::{Chain, Graph},
        prelude::AcalaDexExecutor,
    };
    use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};
    use phat_offchain_rollup::clients::substrate::{
        claim_name, get_name_owner, SubstrateRollupClient,
    };
    use pink_extension::ResultExt;
    use scale::{Decode, Encode};

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        BadOrigin,
        NotConfigured,
        MissingPalletId,
        ChainNotFound,
        WorkerNotFound,
        FailedToGetNameOwner,
        RollupNotConfigured,
        RollupConfiguredByAnotherAccount,
        FailedToClaimName,
        FailedToCreateClient,
        FailedToCommitTx,
        FailedToSendTransaction,
        FailedToFetchTask,
        FailedToInitTask,
        ReadCacheFailed,
        WriteCacheFailed,
        DecodeCacheFailed,
        DecodeGraphFailed,
        SetGraphFailed,
        TaskNotFoundInCache,
        TaskNotFoundOnChain,
        Unimplemented,
    }

    type Result<T> = core::result::Result<T, Error>;

    #[derive(Clone, Encode, Decode, Debug, PackedLayout, SpreadLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Config {
        /// The rollup anchor pallet id on the target blockchain
        rollup_pallet_id: u8,
        /// The endpoint of rollup deployed chain
        rollup_endpoint: String,
    }

    /// Event emitted when graph is set.
    #[ink(event)]
    pub struct GraphSet;

    /// Event emitted when executor is configured.
    #[ink(event)]
    pub struct Configured;

    /// Event emitted when worker account is set to rollup.
    #[ink(event)]
    pub struct WorkerSetToRollup;

    #[derive(Clone, Encode, Decode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum RunningType {
        // [source_chain, worker_sr25519_pub_key]
        Fetch(String, [u8; 32]),
        Execute,
    }

    const SUB_ROLLUP_PREFIX: &[u8] = b"q/";

    #[ink(storage)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Executor {
        pub admin: AccountId,
        pub config: Option<Config>,
        pub graph: Vec<u8>,
        pub worker_prv_keys: Vec<[u8; 32]>,
        pub worker_accounts: Vec<AccountInfo>,
        pub executor_account: [u8; 32],
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
            let mut worker_prv_keys: Vec<[u8; 32]> = vec![];
            let mut worker_accounts: Vec<AccountInfo> = vec![];

            for index in 0..1 {
                let private_key = pink_web3::keys::pink::KeyPair::derive_keypair(
                    &[b"worker".to_vec(), [index].to_vec()].concat(),
                )
                .private_key();
                worker_prv_keys.push(private_key);
                worker_accounts.push(AccountInfo::from(private_key));
            }

            Self {
                admin: Self::env().caller(),
                config: None,
                graph: Vec::default(),
                worker_prv_keys,
                worker_accounts,
                executor_account: pink_web3::keys::pink::KeyPair::derive_keypair(b"executor")
                    .private_key(),
            }
        }

        /// Debug only, remove before merge
        #[ink(message)]
        pub fn import_executor(&mut self, executor: Vec<u8>) -> Result<()> {
            self.ensure_owner()?;
            self.executor_account = executor.try_into().unwrap();
            Ok(())
        }

        /// Debug only, remove before merge
        #[ink(message)]
        pub fn export_executor(&self) -> Result<[u8; 32]> {
            self.ensure_owner()?;
            Ok(self.executor_account)
        }

        /// Debug only, remove before merge
        #[ink(message)]
        pub fn import_workers(&mut self, worker: Vec<u8>) -> Result<()> {
            self.ensure_owner()?;
            let mut worker_prv_keys: Vec<[u8; 32]> = vec![];
            let mut worker_accounts: Vec<AccountInfo> = vec![];

            let private_key: [u8; 32] = worker.try_into().unwrap();
            worker_prv_keys.push(private_key);
            worker_accounts.push(AccountInfo::from(private_key));

            self.worker_prv_keys = worker_prv_keys;
            self.worker_accounts = worker_accounts;
            Ok(())
        }

        /// Debug only, remove before merge
        #[ink(message)]
        pub fn export_workers(&self) -> Result<Vec<[u8; 32]>> {
            self.ensure_owner()?;
            Ok(self.worker_prv_keys.clone())
        }

        #[ink(message)]
        pub fn config(&mut self, rollup_pallet_id: u8, rollup_endpoint: String) -> Result<()> {
            self.ensure_owner()?;
            // Insert empty record in advance
            let empty_tasks: Vec<TaskId> = vec![];
            pink_extension::ext()
                .cache_set(b"running_tasks", &empty_tasks.encode())
                .unwrap();
            self.config = Some(Config {
                rollup_pallet_id,
                rollup_endpoint,
            });
            pink_extension::debug!("Set config as: {:?}", &self.config);
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

        #[ink(message)]
        pub fn setup_rollup(&self) -> Result<()> {
            self.ensure_owner()?;
            let config = self.ensure_configured()?;

            let contract_id = self.env().account_id();
            // Check if the rollup is initialized properly
            let actual_owner = get_name_owner(&config.rollup_endpoint, &contract_id)
                .log_err("failed to get name owner")
                .or(Err(Error::FailedToGetNameOwner))?;
            if let Some(owner) = actual_owner {
                let pubkey = pink_extension::ext().get_public_key(
                    pink_extension::chain_extension::SigType::Sr25519,
                    &self.executor_account,
                );
                if owner.encode() != pubkey {
                    return Err(Error::RollupConfiguredByAnotherAccount);
                }
            } else {
                // Not initialized. Let's claim the name.
                pink_extension::debug!(
                    "Start to claim rollup name for contract id: {:?}",
                    &contract_id
                );
                claim_name(
                    &config.rollup_endpoint,
                    config.rollup_pallet_id,
                    &contract_id,
                    &self.executor_account,
                )
                .log_err("failed to claim name")
                .map(|tx_hash| {
                    pink_extension::debug!(
                        "Send transaction to claim name: {:?}",
                        hex::encode(tx_hash)
                    );
                    // Do nothing so far
                })
                .or(Err(Error::FailedToClaimName))?;
            }

            Ok(())
        }

        #[ink(message)]
        pub fn setup_worker_accounts(&self) -> Result<()> {
            let config = self.ensure_configured()?;
            let contract_id = self.env().account_id();
            let mut client = SubstrateRollupClient::new(
                &config.rollup_endpoint,
                config.rollup_pallet_id,
                &contract_id,
                SUB_ROLLUP_PREFIX,
            )
            .log_err("failed to create rollup client")
            .or(Err(Error::FailedToCreateClient))?;

            // Setup worker accounts if it hasn't been set yet.
            if OnchainAccounts::lookup_free_accounts(&mut client).is_none() {
                pink_extension::debug!(
                    "No onchain worker account exist, start setting throug rollup"
                );
                OnchainAccounts::set_worker_accounts(
                    &mut client,
                    self.worker_accounts
                        .clone()
                        .into_iter()
                        .map(|account| account.account32)
                        .collect(),
                );
                // Submit the transaction if it's not empty
                let maybe_submittable = client
                    .commit()
                    .log_err("failed to commit")
                    .or(Err(Error::FailedToCommitTx))?;

                // Submit to blockchain
                if let Some(submittable) = maybe_submittable {
                    let tx_id = submittable
                        .submit(&self.executor_account, 0)
                        .log_err("failed to submit rollup tx")
                        .or(Err(Error::FailedToSendTransaction))?;
                    pink_extension::debug!(
                        "Send transaction to set worker account: {:?}",
                        hex::encode(tx_id)
                    );
                }
            }
            Self::env().emit_event(WorkerSetToRollup {});
            Ok(())
        }

        #[ink(message)]
        pub fn run(&self, running_type: RunningType) -> Result<()> {
            let config = self.ensure_configured()?;
            let contract_id = self.env().account_id();
            let mut client = SubstrateRollupClient::new(
                &config.rollup_endpoint,
                config.rollup_pallet_id,
                &contract_id,
                SUB_ROLLUP_PREFIX,
            )
            .log_err("failed to create rollup client")
            .or(Err(Error::FailedToCreateClient))?;

            match running_type {
                RunningType::Fetch(source_chain, worker) => {
                    self.fetch_task(&mut client, source_chain, worker)?
                }
                RunningType::Execute => self.execute_task(&mut client)?,
            };

            // Submit the transaction if it's not empty
            let maybe_submittable = client
                .commit()
                .log_err("failed to commit")
                .or(Err(Error::FailedToCommitTx))?;

            // Submit to blockchain
            if let Some(submittable) = maybe_submittable {
                let tx_id = submittable
                    .submit(&self.executor_account, 0)
                    .log_err("failed to submit rollup tx")
                    .or(Err(Error::FailedToSendTransaction))?;
                pink_extension::debug!(
                    "Send transaction to update rollup storage: {:?}",
                    hex::encode(tx_id)
                );
            }
            Ok(())
        }

        /// Return config information
        #[ink(message)]
        pub fn get_config(&self) -> Result<Option<Config>> {
            Ok(self.config.clone())
        }

        /// For cross-contract call test
        #[ink(message)]
        pub fn get_local_tasks(&self) -> Result<Vec<Task>> {
            let mut task_list: Vec<Task> = vec![];
            let local_tasks = pink_extension::ext()
                .cache_get(b"running_tasks")
                .ok_or(Error::ReadCacheFailed)?;
            let decoded_tasks: Vec<TaskId> = Decode::decode(&mut local_tasks.as_slice())
                .map_err(|_| Error::DecodeCacheFailed)?;
            for task_id in decoded_tasks {
                task_list.push(TaskCache::get_task(&task_id).ok_or(Error::TaskNotFoundInCache)?);
            }
            Ok(task_list)
        }

        /// Returs the interior graph, callable to all
        #[ink(message)]
        pub fn get_graph(&self) -> Result<RegistryGraph> {
            let graph: Graph =
                Decode::decode(&mut self.graph.as_slice()).map_err(|_| Error::DecodeGraphFailed)?;
            Ok(graph.into())
        }

        /// Return executor account information
        #[ink(message)]
        pub fn get_executor_account(&self) -> AccountInfo {
            self.executor_account.into()
        }

        /// Return worker accounts information
        #[ink(message)]
        pub fn get_worker_account(&self) -> Vec<AccountInfo> {
            self.worker_accounts.clone()
        }

        /// Return worker accounts information
        #[ink(message)]
        pub fn get_free_worker_account(&self) -> Result<Option<Vec<[u8; 32]>>> {
            let config = self.ensure_configured()?;
            let contract_id = self.env().account_id();
            let mut client = SubstrateRollupClient::new(
                &config.rollup_endpoint,
                config.rollup_pallet_id,
                &contract_id,
                SUB_ROLLUP_PREFIX,
            )
            .log_err("failed to create rollup client")
            .or(Err(Error::FailedToCreateClient))?;
            Ok(OnchainAccounts::lookup_free_accounts(&mut client))
        }

        /// Search actived tasks from source chain and upload them to rollup storage
        pub fn fetch_task(
            &self,
            client: &mut SubstrateRollupClient,
            source_chain: String,
            // Worker sr25519 public key
            worker: [u8; 32],
        ) -> Result<()> {
            pink_extension::debug!("Fetch actived task from {:?}", &source_chain);
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
        pub fn execute_task(&self, client: &mut SubstrateRollupClient) -> Result<()> {
            let bridge_executors = self.create_bridge_executors()?;
            let dex_executors = self.create_dex_executors()?;

            for id in OnchainTasks::lookup_pending_tasks(client).iter() {
                pink_extension::debug!(
                    "Found one pending tasks exist in rollup storge, task id: {:?}",
                    &hex::encode(id)
                );
                // Get task saved in local cache, if not exist in local, try recover from on-chain storage
                // FIXME: First time execute the task, it would be treat as broken, then trying to recover
                let mut task = TaskCache::get_task(id)
                    .or_else(|| {
                        pink_extension::warn!("Task data lost in local cache unexpectedly, try recover from rollup storage, task id: {:?}", &hex::encode(id));
                        if let Some(mut onchain_task) = OnchainTasks::lookup_task(client, id) {
                            // The state of task saved in rollup storage is `Initialized`, to understand
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
                    },
                    client,
                ) {
                    Ok(TaskStatus::Completed) => {
                        pink_extension::info!(
                            "Task execution completed, delete it from rollup storage: {:?}",
                            hex::encode(task.id)
                        );
                        // Remove task from blockchain and recycle worker account
                        task.destroy(client)
                            .map_err(|_| Error::RollupNotConfigured)?;
                        // If task already delete from rollup storage, delete it from local cache
                        if OnchainTasks::lookup_task(client, id).is_none() {
                            pink_extension::info!(
                                "Task delete from rollup storage, remove it from local cache: {:?}",
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
                            "Task execution has not finished yet, update data to local cache: {:?}",
                            hex::encode(task.id)
                        );
                        TaskCache::update_task(&task).map_err(|_| Error::WriteCacheFailed)?;
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

            let moonbeam_xtoken: [u8; 20] =
                hex_literal::hex!("0000000000000000000000000000000000000804");

            // Moonbeam -> Acala
            bridge_executors.push((
                (String::from("Moonbeam"), String::from("Acala")),
                Box::new(Moonbeam2AcalaExecutor::new(
                    &moonbeam.endpoint,
                    moonbeam_xtoken.into(),
                )),
            ));
            // Moonbeam -> Phala
            bridge_executors.push((
                (String::from("Moonbeam"), String::from("Phala")),
                Box::new(Moonbeam2PhalaExecutor::new(
                    &moonbeam.endpoint,
                    moonbeam_xtoken.into(),
                )),
            ));
            // Phala -> Acala
            bridge_executors.push((
                (String::from("Phala"), String::from("Acala")),
                Box::new(Phala2AcalaExecutor::new(&phala.endpoint)),
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
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        // use dotenv::dotenv;
        use ink_lang as ink;
        use phala_pallet_common::WrapSlice;
        // use pink_extension::PinkEnvironment;
        use xcm::latest::{prelude::*, MultiLocation};

        fn deploy_executor() -> ExecutorRef {
            // Register contracts
            let hash = ink_env::Hash::try_from([20u8; 32]).unwrap();
            ink_env::test::register_contract::<Executor>(hash.as_ref());

            // Insert empty record in advance
            let empty_tasks: Vec<TaskId> = vec![];
            pink_extension::ext()
                .cache_set(b"running_tasks", &empty_tasks.encode())
                .unwrap();

            // Deploy Executor
            ExecutorRef::default()
                .code_hash(hash)
                .endowment(0)
                .salt_bytes([0u8; 0])
                .instantiate()
                .expect("failed to deploy Executor")
        }

        #[ignore]
        #[ink::test]
        fn rollup_should_work() {
            pink_extension_runtime::mock_ext::mock_all_ext();
            let mut executor = deploy_executor();
            // Initial rollup
            assert_eq!(
                executor.config(100, String::from("http://127.0.0.1:39933")),
                Ok(())
            );
            assert_eq!(executor.setup_rollup(), Ok(()));
        }

        #[ignore]
        #[ink::test]
        fn setup_worker_accounts_should_work() {
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
            // Initial rollup
            assert_eq!(
                executor.config(100, String::from("http://127.0.0.1:39933")),
                Ok(())
            );
            assert_eq!(executor.setup_rollup(), Ok(()));
            assert_eq!(executor.setup_worker_accounts(), Ok(()));
            let onchain_free_accounts = executor.get_free_worker_account().unwrap().unwrap();
            let local_worker_accounts: Vec<[u8; 32]> = executor
                .get_worker_account()
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
