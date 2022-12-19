#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use ink_lang as ink;

mod task;
mod task_uploader;
mod types;

#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_executor {
    use alloc::{
        string::{String, ToString},
        vec,
        vec::Vec,
    };
    use scale::{Decode, Encode};
    // use index::{ensure, prelude::*};
    use index::utils::ToArray;
    use index_registry::{types::Graph, RegistryRef};
    use ink_env::call::FromAccountId;
    use phat_offchain_rollup::{
        clients::substrate::{claim_name, get_name_owner, SubstrateRollupClient},
        Action,
    };
    use pink_extension::chain_extension::{signing, SigType};
    // To enable `(result).log_err("Reason")?`
    use crate::task_uploader::UploadToChain;
    use crate::types::{Task, TaskId, TaskStatus};
    use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};
    use pink_extension::ResultExt;

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
        ReadCacheFailed,
        WriteCacheFailed,
        DecodeCacheFailed,
        TaskNotFoundInCache,
        ExecuteFailed,
        Unimplemented,
    }

    type Result<T> = core::result::Result<T, Error>;

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct AccountInfo {
        pub account32: [u8; 32],
        pub account20: [u8; 20],
    }

    #[derive(Encode, Decode, Debug, PackedLayout, SpreadLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    struct Config {
        /// Registry contract
        registry: RegistryRef,
        /// The rollup anchor pallet id on the target blockchain
        pallet_id: u8,
    }

    impl From<[u8; 32]> for AccountInfo {
        fn from(privkey: [u8; 32]) -> Self {
            let ecdsa_pubkey: [u8; 33] = signing::get_public_key(&privkey, SigType::Ecdsa)
                .try_into()
                .expect("Public key should be of length 33");
            let mut ecdsa_address = [0u8; 20];
            ink_env::ecdsa_to_eth_address(&ecdsa_pubkey, &mut ecdsa_address)
                .expect("Get address of ecdsa failed");
            Self {
                account32: signing::get_public_key(&privkey, SigType::Sr25519).to_array(),
                account20: ecdsa_address,
            }
        }
    }

    #[ink(storage)]
    // #[derive(SpreadAllocate)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Executor {
        admin: AccountId,
        config: Option<Config>,
        worker_accounts: Vec<[u8; 32]>,
        executor_account: [u8; 32],
    }

    impl Default for Executor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Executor {
        /// Create an Executor entity
        #[ink(constructor)]
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

            // Create worker account
            let empty_alloc: Vec<[u8; 32]> = vec![];
            pink_extension::ext()
                .cache_set(b"alloc", &empty_alloc.encode())
                .expect("write cache failed");

            Self {
                admin: Self::env().caller(),
                config: None,
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

        /// Claim and execute tasks from all supported blockchains. This is a query operation
        /// that scheduler invokes periodically.
        ///
        ///
        /// 1) Perform spcific operations for the runing tasks according to current status.
        /// 2) Fetch new actived tasks from supported chains and append them to the local runing tasks queue.
        ///
        #[ink(message)]
        pub fn execute(&self) -> Result<()> {
            Err(Error::Unimplemented)
        }

        /// Claim tasks from source chain. The worflow is:
        /// 1. Get actived task from all supported chains
        /// 2. Upload task info to rollup storage
        #[ink(message)]
        pub fn upload_task(&self, _source_chain: String) -> Result<()> {
            let config = self.ensure_configured()?;

            // TODO: fetch actived tasks from supported blockchain

            // TODO: Maybe recover cache through onchain storage if it is empty

            // Get rpc info from registry
            let chain = config
                .registry
                .clone()
                .get_chain("Khala".to_string())
                .map_err(|_| Error::ChainNotFound)?;
            let contract_id = self.env().account_id();
            let mut client =
                SubstrateRollupClient::new(&chain.endpoint, config.pallet_id, &contract_id)
                    .log_err("failed to create rollup client")
                    .or(Err(Error::FailedToCreateClient))?;

            // TODO: Use real task data
            let mut task = Task {
                id: [0; 32],
                worker: [0; 32],
                status: TaskStatus::Initialized,
                source: b"Ethereum".to_vec(),
                edges: vec![],
                sender: vec![],
                recipient: vec![],
            };
            // TODO: Use session save task/taskid_list/task_worker_account info.
            client.action(Action::Reply(UploadToChain { task: task.clone() }.encode()));
            // Submit the transaction if it's not empty
            let maybe_submittable = client
                .commit()
                .log_err("failed to commit")
                .or(Err(Error::FailedToCommitTx))?;
            if let Some(submittable) = maybe_submittable {
                let tx_id = submittable
                    .submit(&self.executor_account, 0)
                    .log_err("failed to submit rollup tx")
                    .or(Err(Error::FailedToSendTransaction))?;

                // Add the new task to local cache
                task.status = TaskStatus::Uploading(Some(tx_id));
                // Save for debug purpose, will remove
                self.add_task(&task)?;
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
                task_list.push(self.get_task(&task_id).unwrap());
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
            let mut accounts: Vec<AccountInfo> = Vec::new();
            for worker in &self.worker_accounts {
                accounts.push((*worker).into())
            }
            accounts
        }

        /// Mark an account as allocated, e.g. put it into local cache `alloc` queue.
        #[allow(dead_code)]
        fn allocate_worker(&self, worker: &[u8; 32]) -> Result<()> {
            let alloc_list = pink_extension::ext()
                .cache_get(b"alloc")
                .ok_or(Error::ReadCacheFailed)?;
            let mut decoded_list: Vec<[u8; 32]> =
                Decode::decode(&mut alloc_list.as_slice()).map_err(|_| Error::DecodeCacheFailed)?;

            decoded_list.push(*worker);
            pink_extension::ext()
                .cache_set(b"alloc", &decoded_list.encode())
                .map_err(|_| Error::WriteCacheFailed)?;
            Ok(())
        }

        /// Retuen accounts that hasn't been allocated to a specific task
        #[allow(dead_code)]
        fn free_worker(&self) -> Result<Vec<[u8; 32]>> {
            let mut free_list = vec![];
            let alloc_list = pink_extension::ext()
                .cache_get(b"alloc")
                .ok_or(Error::ReadCacheFailed)?;
            let decoded_list: Vec<[u8; 32]> =
                Decode::decode(&mut alloc_list.as_slice()).map_err(|_| Error::DecodeCacheFailed)?;

            for worker in self.worker_accounts.iter() {
                if !decoded_list.contains(worker) {
                    free_list.push(*worker);
                }
            }
            Ok(free_list)
        }

        fn add_task(&self, task: &Task) -> Result<()> {
            let local_tasks = pink_extension::ext()
                .cache_get(b"running_tasks")
                .ok_or(Error::ReadCacheFailed)?;
            let mut decoded_tasks: Vec<TaskId> = Decode::decode(&mut local_tasks.as_slice())
                .map_err(|_| Error::DecodeCacheFailed)?;

            if !decoded_tasks.contains(&task.id) {
                decoded_tasks.push(task.id);
                pink_extension::ext()
                    .cache_set(b"running_tasks", &decoded_tasks.encode())
                    .map_err(|_| Error::WriteCacheFailed)?;
                // Save full task information
                pink_extension::ext()
                    .cache_set(&task.id, &task.encode())
                    .map_err(|_| Error::WriteCacheFailed)?;
            }
            Ok(())
        }

        fn remove_task(&self, task: &Task) -> Result<()> {
            let local_tasks = pink_extension::ext()
                .cache_get(b"running_tasks")
                .ok_or(Error::ReadCacheFailed)?;
            let mut decoded_tasks: Vec<TaskId> = Decode::decode(&mut local_tasks.as_slice())
                .map_err(|_| Error::DecodeCacheFailed)?;
            let index = decoded_tasks
                .iter()
                .position(|id| *id == task.id)
                .ok_or(Error::TaskNotFoundInCache)?;
            decoded_tasks.remove(index);
            // Delete task record from cache
            pink_extension::ext()
                .cache_remove(&task.id)
                .ok_or(Error::WriteCacheFailed)?;

            Ok(())
        }

        fn update_task(&self, task: &Task) -> Result<()> {
            if let Some(_) = pink_extension::ext().cache_get(&task.id) {
                // Update task record
                pink_extension::ext()
                    .cache_set(&task.id, &task.encode())
                    .map_err(|_| Error::WriteCacheFailed)?;
            }
            Ok(())
        }

        fn get_task(&self, id: &TaskId) -> Option<Task> {
            pink_extension::ext()
                .cache_get(id)
                .and_then(|encoded_task| {
                    match Decode::decode(&mut encoded_task.as_slice())
                        .map_err(|_| Error::DecodeCacheFailed)
                    {
                        Ok(task) => Some(task),
                        _ => None,
                    }
                })
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
                .expect("failed to deploy EvmTransactor");
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
                .expect("failed to deploy SampleOracle");
            assert_eq!(executor.set_registry(registry.to_account_id()), Ok(()));

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
        fn worker_allocation_cache_should_work() {
            use pink_extension::chain_extension::mock;
            use std::{cell::RefCell, collections::HashMap, rc::Rc};

            pink_extension_runtime::mock_ext::mock_all_ext();

            let storage: Rc<RefCell<HashMap<Vec<u8>, Vec<u8>>>> = Default::default();

            {
                let storage = storage.clone();
                mock::mock_cache_set(move |k, v| {
                    storage.borrow_mut().insert(k.to_vec(), v.to_vec());
                    Ok(())
                });
            }
            {
                let storage = storage.clone();
                mock::mock_cache_get(move |k| storage.borrow().get(k).map(|v| v.to_vec()));
            }
            {
                let storage = storage.clone();
                mock::mock_cache_remove(move |k| {
                    storage.borrow_mut().remove(k).map(|v| v.to_vec())
                });
            }

            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut executor = Executor::new();
            assert_eq!(executor.free_worker().unwrap(), executor.worker_accounts);
            assert_eq!(
                executor.allocate_worker(&executor.worker_accounts[0]),
                Ok(())
            );
            assert_eq!(
                executor.allocate_worker(&executor.worker_accounts[1]),
                Ok(())
            );
            assert_eq!(
                executor.free_worker().unwrap(),
                executor.worker_accounts[2..]
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
