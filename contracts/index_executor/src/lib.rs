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
    use index_registry::{types::Graph, RegistryRef};
    use ink_env::call::FromAccountId;
    use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};
    use phat_offchain_rollup::clients::substrate::{
        claim_name, get_name_owner, SubstrateRollupClient,
    };
    use pink_extension::ResultExt;
    use scale::{Decode, Encode};

    use crate::account::AccountInfo;
    use crate::cache::*;
    use crate::claimer::ActivedTaskFetcher;
    use crate::context::Context;
    use crate::task::{OnchainTasks, Task, TaskId, TaskStatus};

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
        TaskNotFoundOnChain,
        ExecuteFailed,
        Unimplemented,
    }

    type Result<T> = core::result::Result<T, Error>;

    #[derive(Clone, Encode, Decode, Debug, PackedLayout, SpreadLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Config {
        /// Registry contract
        registry: RegistryRef,
        /// The rollup anchor pallet id on the target blockchain
        pallet_id: u8,
    }

    #[derive(Clone, Encode, Decode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum RunningType {
        // [source_chain]
        Fetch(String),
        Execute,
    }

    #[ink(storage)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Executor {
        pub admin: AccountId,
        pub config: Option<Config>,
        pub worker_prv_keys: Vec<[u8; 32]>,
        pub worker_accounts: Vec<AccountInfo>,
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
            let mut worker_prv_keys: Vec<[u8; 32]> = vec![];
            let mut worker_accounts: Vec<AccountInfo> = vec![];

            for index in 0..10 {
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
                worker_prv_keys,
                worker_accounts,
                executor_account: pink_web3::keys::pink::KeyPair::derive_keypair(b"executor")
                    .private_key(),
            }
        }

        #[ink(message)]
        pub fn config(&mut self, registry: AccountId, pallet_id: u8) -> Result<()> {
            self.ensure_owner()?;
            self.config = Some(Config {
                registry: RegistryRef::from_account_id(registry),
                pallet_id,
            });

            let contract_id = self.env().account_id();
            // Get rpc info from registry
            let chain = self
                .config
                .clone()
                .unwrap()
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
                    self.config.clone().unwrap().pallet_id,
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

        #[ink(message)]
        pub fn run(&self, running_type: RunningType) -> Result<()> {
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

            match running_type {
                RunningType::Fetch(source_chain) => self.fetch_task(&client, source_chain)?,
                RunningType::Execute => self.execute_task(&client)?,
            };

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
                task_list.push(TaskCache::get_task(&task_id).unwrap());
            }
            Ok(task_list)
        }

        /// For cross-contract call test
        #[ink(message)]
        pub fn get_graph(&self) -> Result<Graph> {
            let config = self.ensure_configured()?;
            let graph = config.registry.clone().get_graph().unwrap();
            Ok(graph)
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

        /// Search actived tasks from source chain and upload them to rollup storage
        pub fn fetch_task(
            &self,
            client: &SubstrateRollupClient,
            source_chain: String,
        ) -> Result<()> {
            // Fetch actived task that completed initial confirmation from specific chain that belong to current worker,
            // and append them to runing tasks
            let mut actived_task = ActivedTaskFetcher::new(
                self.config
                    .clone()
                    .unwrap()
                    .registry
                    .clone()
                    .get_chain(source_chain)
                    .unwrap(),
                AccountInfo::from(self.executor_account),
            )
            .fetch_task()
            .map_err(|_| Error::FailedToFetchTask)?;

            // Initialize task, and save it to on-chain storage
            actived_task.init(
                &Context {
                    // Don't need signer here
                    signer: [0; 32],
                    registry: self.config.clone().unwrap().registry.clone(),
                    worker_accounts: self.worker_accounts.clone(),
                },
                &client,
            );

            Ok(())
        }

        /// Execute tasks from all supported blockchains. This is a query operation
        /// that scheduler invokes periodically.
        pub fn execute_task(&self, client: &SubstrateRollupClient) -> Result<()> {
            for id in OnchainTasks::lookup_pending_tasks(&client).iter() {
                // Get task saved in local cache, if not exist in local, try recover from on-chain storage
                let mut task = TaskCache::get_task(&id)
                    .or_else(|| {
                        if let Some(onchain_task) = OnchainTasks::lookup_task(&client, &id) {
                            onchain_task.sync(&client);
                            // Add task to local cache
                            let _ = TaskCache::add_task(&onchain_task);
                            Some(onchain_task)
                        } else {
                            None
                        }
                    })
                    .ok_or(Error::TaskNotFoundOnChain)?;

                match task.execute(&Context {
                    signer: self.pub_to_prv(task.worker).unwrap(),
                    registry: self.config.clone().unwrap().registry.clone(),
                    worker_accounts: self.worker_accounts.clone(),
                }) {
                    Ok(TaskStatus::Completed) => {
                        // Remove task from blockchain and recycle worker account
                        task.destroy(&client);
                        // If task already delete from rollup storage, delete it from local cache
                        if OnchainTasks::lookup_task(&client, &id) == None {
                            TaskCache::remove_task(&task).map_err(|_| Error::WriteCacheFailed)?;
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
                        TaskCache::update_task(&task).map_err(|_| Error::WriteCacheFailed)?;
                        continue;
                    }
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

        fn pub_to_prv(&self, pub_key: [u8; 32]) -> Option<[u8; 32]> {
            self.worker_accounts
                .iter()
                .position(|a| a.account32 == pub_key)
                .map(|idx| self.worker_prv_keys[idx].clone())
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
            // Initial rollup
            assert_eq!(executor.config(registry.to_account_id(), 100), Ok(()));
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
