#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use ink_lang as ink;

mod account;
mod bridge;
mod cache;
mod claimer;
mod context;
mod step;
mod swap;
mod task;
mod traits;

#[allow(clippy::large_enum_variant)]
#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_executor {
    use alloc::{
        string::{String, ToString},
        vec,
        vec::Vec,
    };
    use index::{ensure, prelude::*};
    use index_registry::{
        types::{ChainType, NonceFetcher},
        RegistryRef,
    };
    use ink_env::call::FromAccountId;
    use ink_storage::{
        traits::{PackedLayout, SpreadAllocate, SpreadLayout, StorageLayout},
        Mapping,
    };
    use phat_offchain_rollup::clients::substrate::{
        claim_name, get_name_owner, SubstrateRollupClient,
    };
    use pink_extension::ResultExt;
    use scale::{Decode, Encode};

    use crate::account::AccountInfo;
    use crate::cache::*;
    use crate::claimer::ActivedTaskFetcher;
    use crate::context::Context;
    use crate::task::{Task, TaskId, TaskStatus};

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        BadOrigin,
        NotConfigured,
        ChainNotFound,
        FailedToGetNameOwner,
        RollupConfiguredByAnotherAccount,
        FailedToClaimName,
        FailedToCreateClient,
        FailedToCommitTx,
        FailedToSendTransaction,
        FailedToFetchTask,
        ReadCacheFailed,
        WriteCacheFailed,
        DecodeCacheFailed,
        TaskNotFoundInCache,
        ExecuteFailed,
        Unimplemented,
    }

    type Result<T> = core::result::Result<T, Error>;

    #[derive(Encode, Decode, Debug, PackedLayout, SpreadLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Config {
        /// Registry contract
        registry: RegistryRef,
        /// The rollup anchor pallet id on the target blockchain
        pallet_id: u8,
    }

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Executor {
        pub admin: AccountId,
        pub config: Option<Config>,
        pub worker_accounts: Vec<[u8; 32]>,
        pub executor_account: [u8; 32],
        pub pub_to_prv: Mapping<[u8; 32], [u8; 32]>,
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
            ink_lang::utils::initialize_contract(|this: &mut Self| {
                for index in 0..10 {
                    let private_key = pink_web3::keys::pink::KeyPair::derive_keypair(
                        &[b"worker".to_vec(), [index].to_vec()].concat(),
                    )
                    .private_key();
                    this.worker_accounts.push(private_key);
                    this.pub_to_prv
                        .insert(&AccountInfo::from(private_key).account32, &private_key);
                }

                this.admin = Self::env().caller();
                this.config = None;
                this.executor_account =
                    pink_web3::keys::pink::KeyPair::derive_keypair(b"executor").private_key();
            })
        }

        #[ink(message)]
        pub fn config(&mut self, registry: AccountId, pallet_id: u8) -> Result<()> {
            self.ensure_owner()?;
            self.config = Some(Config {
                registry: RegistryRef::from_account_id(registry),
                pallet_id,
            });
            Ok(())
        }

        /// Initialize rollup after registry set
        /// executor account key will be the key that submit transaction to target blockchains
        #[ink(message)]
        pub fn init_rollup(&self) -> Result<()> {
            let config = self.ensure_configured()?;
            let contract_id = self.env().account_id();
            // Get rpc info from registry
            let chain = config
                .registry
                .clone()
                .get_chain("Khala".to_string())
                .map_err(|_| Error::ChainNotFound)?;
            let endpoint = chain.endpoint;
            // Check if the rollup is initialized properly
            let actual_owner = get_name_owner(&endpoint, &contract_id)
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
                claim_name(
                    &endpoint,
                    config.pallet_id,
                    &contract_id,
                    &self.executor_account,
                )
                .log_err("failed to claim name")
                .map(|_tx_hash| {
                    // Do nothing so far
                })
                .or(Err(Error::FailedToClaimName))?;
            }
            Ok(())
        }

        /// Search actived tasks from source chain and upload them to rollup storage
        #[ink(message)]
        pub fn fetch_task(&self, source_chain: String) -> Result<()> {
            let config = self.ensure_configured()?;
            // Get rpc info from registry
            let chain = config
                .registry
                .clone()
                .get_chain("Khala".to_string())
                .map_err(|_| Error::ChainNotFound)?;
            let contract_id = self.env().account_id();
            let client =
                SubstrateRollupClient::new(&chain.endpoint, config.pallet_id, &contract_id)
                    .log_err("failed to create rollup client")
                    .or(Err(Error::FailedToCreateClient))?;

            // Fetch actived task that completed initial confirmation from specific chain that belong to current worker,
            // and append them to runing tasks
            let mut onchain_actived_tasks = ActivedTaskFetcher::new(
                config.registry.clone().get_chain(source_chain).unwrap(),
                AccountInfo::from(self.executor_account),
            )
            .fetch_tasks()
            .map_err(|_| Error::FailedToFetchTask)?;

            self.initialize_task_onchain(&client, &mut onchain_actived_tasks);

            // Submit the transaction if it's not empty
            let maybe_submittable = client
                .commit()
                .log_err("failed to commit")
                .or(Err(Error::FailedToCommitTx))?;

            // Submit to blockchain
            if let Some(submittable) = maybe_submittable {
                let _tx_id = submittable
                    .submit(&self.executor_account, 0)
                    .log_err("failed to submit rollup tx")
                    .or(Err(Error::FailedToSendTransaction))?;
            }

            Ok(())
        }

        /// Execute tasks from all supported blockchains. This is a query operation
        /// that scheduler invokes periodically.
        #[ink(message)]
        pub fn execute_task(&self) -> Result<()> {
            let config = self.ensure_configured()?;
            // Get rpc info from registry
            let chain = config
                .registry
                .clone()
                .get_chain("Khala".to_string())
                .map_err(|_| Error::ChainNotFound)?;
            let contract_id = self.env().account_id();
            let client =
                SubstrateRollupClient::new(&chain.endpoint, config.pallet_id, &contract_id)
                    .log_err("failed to create rollup client")
                    .or(Err(Error::FailedToCreateClient))?;

            // Try recover cache from onchain storage if it is empty or crashed
            self.maybe_recover_cache(&client)?;

            let local_tasks = get_all_task_local().map_err(|_| Error::ReadCacheFailed)?;

            for id in local_tasks.iter() {
                // Get task saved in local cache
                let task = get_task_local(&id).ok_or(Error::ExecuteFailed)?;

                match task.execute_next(&Context {
                    signer: self.pub_to_prv.get(&task.worker).unwrap(),
                    registry: config.registry.clone(),
                }) {
                    Ok(TaskStatus::Completed) => {
                        // Remove task from blockchain and recycle worker account
                        self.destroy_task_onchain(&client, &task);
                        // If task already delete from rollup storage, delete it from local cache
                        if self.lookup_task_onchain(&client, task.id) == None {
                            remove_task_local(&task).map_err(|_| Error::WriteCacheFailed)?;
                        }
                    }
                    Err(_) => {
                        // Execution failed, prepare necessary informations that DAO can handle later.
                        // Informatios should contains:
                        // 1. Sender on source chain
                        // 2. Current step
                        // 3. The allocated worker account
                        // 4. Current asset that worker account hold
                        //
                    }
                    _ => {
                        update_task_local(&task).map_err(|_| Error::WriteCacheFailed)?;
                        continue;
                    }
                }
            }

            // Submit the transaction if it's not empty
            let maybe_submittable = client
                .commit()
                .log_err("failed to commit")
                .or(Err(Error::FailedToCommitTx))?;

            // Submit to blockchain
            if let Some(submittable) = maybe_submittable {
                let _tx_id = submittable
                    .submit(&self.executor_account, 0)
                    .log_err("failed to submit rollup tx")
                    .or(Err(Error::FailedToSendTransaction))?;
            }

            Ok(())
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
                task_list.push(get_task_local(&task_id).unwrap());
            }
            Ok(task_list)
        }

        /// Return executor account information
        #[ink(message)]
        pub fn get_executor_account(&self) -> AccountInfo {
            self.executor_account.into()
        }

        fn initialize_task_onchain(&self, client: &SubstrateRollupClient, tasks: &mut Vec<Task>) {
            let mut free_accounts = self.lookup_free_accounts_onchain(client);
            let mut pending_tasks = self.lookup_pending_tasks_onchain(client);

            for task in tasks.iter_mut() {
                if self.lookup_task_onchain(&client, task.id).is_some() {
                    // Task already saved, skip
                    continue;
                }
                if let Some(account) = free_accounts.pop() {
                    // Apply a worker account
                    task.worker = account;
                    // Aplly worker nonce for each step in task
                    self.aplly_nonce(client, task);
                    // TODO: query initial balance of worker account and setup to specific step
                    task.status = TaskStatus::Initialized;
                    // Push to pending tasks queue
                    pending_tasks.push(task.id);
                    // Save task data
                    // client.session.put(task.id, task);
                } else {
                    // We can not handle more tasks any more
                    break;
                }
            }

            // client.session.put(b"free_accounts".to_vec(), free_accounts);
            // client.session.put(b"pending_tasks".to_vec(), pending_tasks);
            // client.commit();
        }

        fn destroy_task_onchain(&self, client: &SubstrateRollupClient, task: &Task) {
            let mut free_accounts = self.lookup_free_accounts_onchain(client);
            let mut pending_tasks = self.lookup_pending_tasks_onchain(client);

            if self.lookup_task_onchain(&client, task.id).is_some() {
                if let Some(idx) = pending_tasks.iter().position(|id| *id == task.id) {
                    // Remove from pending tasks queue
                    pending_tasks.remove(idx);
                    // Recycle worker account
                    free_accounts.push(task.worker);
                    // Delete task data
                    // client.session.remove(task.id);
                }
                // client.session.put(b"free_accounts".to_vec(), free_accounts);
                // client.session.put(b"pending_tasks".to_vec(), pending_tasks);
                // client.commit();
            }
        }

        fn lookup_task_onchain(&self, client: &SubstrateRollupClient, id: TaskId) -> Option<Task> {
            // client.session.get(id)
            None
        }

        fn lookup_free_accounts_onchain(&self, client: &SubstrateRollupClient) -> Vec<[u8; 32]> {
            // let free_accounts: Vec<[u8; 32]> =
            // client.session.get(b"free_accounts".to_vec()).unwrap();
            // free_accounts
            vec![]
        }

        fn lookup_pending_tasks_onchain(&self, client: &SubstrateRollupClient) -> Vec<TaskId> {
            // let pending_tasks: Vec<TaskId> =
            // client.session.get(b"pending_tasks".to_vec()).unwrap();
            // pending_tasks
            vec![]
        }

        fn aplly_nonce(&self, client: &SubstrateRollupClient, task: &mut Task) {
            let mut nonce_map: Mapping<String, u64> = Mapping::default();
            for step in task.steps.iter_mut() {
                let nonce = nonce_map.get(&step.chain).or_else(|| {
                    let chain = self
                        .config
                        .as_ref()
                        .unwrap()
                        .registry
                        .get_chain(step.chain.clone())
                        .unwrap();
                    let account_info =
                        AccountInfo::from(self.pub_to_prv.get(&task.worker).unwrap());
                    let account = match chain.chain_type {
                        ChainType::Evm => account_info.account20.to_vec(),
                        ChainType::Sub => account_info.account32.to_vec(),
                    };
                    let onchain_nonce = chain.get_nonce(account).ok();
                    onchain_nonce
                });
                step.nonce = nonce;
                // Increase nonce by 1
                nonce_map.insert(step.chain.clone(), &(nonce.unwrap() + 1));
            }
        }

        fn maybe_recover_cache(&self, client: &SubstrateRollupClient) -> Result<()> {
            match get_all_task_local() {
                Ok(runing_tasks) => {
                    // If local cache is empty, try to recover
                    if runing_tasks.len() == 0 {
                        self.recover_from_rollup_storage(client)?;
                    }
                }
                Err(_) => {
                    // If failed to read cache, try to recover
                    self.recover_from_rollup_storage(client)?;
                }
            }
            Ok(())
        }

        fn recover_from_rollup_storage(&self, client: &SubstrateRollupClient) -> Result<()> {
            let empty_tasks: Vec<TaskId> = vec![];

            pink_extension::ext()
                .cache_set(b"running_tasks", &empty_tasks.encode())
                .map_err(|_| Error::WriteCacheFailed)?;

            // Read from rollup storage
            let pending_tasks = self.lookup_pending_tasks_onchain(client);
            for id in pending_tasks {
                if let Some(task) = self.lookup_task_onchain(client, id) {
                    // Recover status of the task
                    task.sync_status();
                    add_task_local(&task).map_err(|_| Error::WriteCacheFailed)?;
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
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use dotenv::dotenv;
        use index_registry::{
            types::{AssetGraph, AssetInfo, ChainInfo, ChainType, Graph},
            Registry,
        };
        use ink::ToAccountId;
        use ink_lang as ink;
        use phala_pallet_common::WrapSlice;
        use pink_extension::PinkEnvironment;
        use xcm::latest::{prelude::*, MultiLocation};

        fn default_accounts() -> ink_env::test::DefaultAccounts<PinkEnvironment> {
            ink_env::test::default_accounts::<PinkEnvironment>()
        }

        fn set_caller(sender: AccountId) {
            ink_env::test::set_caller::<PinkEnvironment>(sender);
        }

        #[ink::test]
        fn crosscontract_call_should_work() {
            pink_extension_runtime::mock_ext::mock_all_ext();

            // Register contracts
            let hash1 = ink_env::Hash::try_from([10u8; 32]).unwrap();
            let hash2 = ink_env::Hash::try_from([20u8; 32]).unwrap();
            ink_env::test::register_contract::<Registry>(hash1.as_ref());
            ink_env::test::register_contract::<Executor>(hash2.as_ref());

            // Deploy Registry
            let mut registry = RegistryRef::new()
                .code_hash(hash1)
                .endowment(0)
                .salt_bytes([0u8; 0])
                .instantiate()
                .expect("failed to deploy Registry");
            let ethereum = ChainInfo {
                name: "Ethereum".to_string(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: "endpoint".to_string(),
                network: None,
            };
            assert_eq!(registry.register_chain(ethereum.clone()), Ok(()));
            let usdc = AssetInfo {
                name: "USD Coin".to_string(),
                symbol: "USDC".to_string(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(
                registry.register_asset("Ethereum".to_string(), usdc.clone()),
                Ok(())
            );

            // Deploy Executor
            let mut executor = ExecutorRef::new()
                .code_hash(hash2)
                .endowment(0)
                .salt_bytes([0u8; 0])
                .instantiate()
                .expect("failed to deploy Executor");
            assert_eq!(executor.config(registry.to_account_id(), 100), Ok(()));

            // Make cross contract call from executor
            assert_eq!(
                executor.get_graph().unwrap(),
                Graph {
                    assets: vec![AssetGraph {
                        chain: ethereum.name,
                        location: usdc.location,
                        name: usdc.name,
                        symbol: usdc.symbol,
                        decimals: usdc.decimals,
                    }],
                    pairs: vec![],
                    bridges: vec![],
                }
            )
        }

        #[ink::test]
        fn rollup_should_work() {
            pink_extension_runtime::mock_ext::mock_all_ext();

            // Register contracts
            let hash1 = ink_env::Hash::try_from([10u8; 32]).unwrap();
            let hash2 = ink_env::Hash::try_from([20u8; 32]).unwrap();
            ink_env::test::register_contract::<Registry>(hash1.as_ref());
            ink_env::test::register_contract::<Executor>(hash2.as_ref());

            // Deploy Registry
            let mut registry = RegistryRef::new()
                .code_hash(hash1)
                .endowment(0)
                .salt_bytes([0u8; 0])
                .instantiate()
                .expect("failed to deploy Registry");
            let khala = ChainInfo {
                name: "Khala".to_string(),
                chain_type: ChainType::Sub,
                native: None,
                stable: None,
                endpoint: "http://127.0.0.1:39933".to_string(),
                network: None,
            };
            assert_eq!(registry.register_chain(khala.clone()), Ok(()));

            // Insert empty record in advance
            let empty_tasks: Vec<TaskId> = vec![];
            pink_extension::ext()
                .cache_set(b"running_tasks", &empty_tasks.encode())
                .unwrap();

            // Deploy Executor
            let mut executor = ExecutorRef::new()
                .code_hash(hash2)
                .endowment(0)
                .salt_bytes([0u8; 0])
                .instantiate()
                .expect("failed to deploy Executor");
            assert_eq!(executor.config(registry.to_account_id(), 100), Ok(()));
            // Initial rollup
            let r = executor.init_rollup().expect("failed to init");
            pink_extension::warn!("init rollup: {r:?}");
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
