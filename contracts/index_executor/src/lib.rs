#![cfg_attr(not(any(feature = "std", test)), no_std)]
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
        boxed::Box,
        string::{String, ToString},
        vec,
        vec::Vec,
    };
    use hex_literal::hex;
    use index::graph::ChainType;
    use index::graph::{Asset, Bridge, BridgePair, Chain, Dex, DexIndexer, DexPair, Graph};
    use index::prelude::*;
    use index_registry::{Graph as RegistryGraph, RegistryRef};
    use ink_env::call::FromAccountId;
    use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};
    use phat_offchain_rollup::clients::substrate::{
        claim_name, get_name_owner, SubstrateRollupClient,
    };
    use pink_extension::ResultExt;
    use primitive_types::{H160, H256};
    use scale::{Decode, Encode};

    use crate::account::AccountInfo;
    use crate::cache::*;
    use crate::claimer::ActivedTaskFetcher;
    use crate::context::Context;
    use crate::task::{OnchainAccounts, OnchainTasks, Task, TaskId, TaskStatus};

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
        pallet_id: Option<u8>,
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
        pub raw_graph: Vec<u8>,
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
                raw_graph: Vec::default(),
                worker_prv_keys,
                worker_accounts,
                executor_account: pink_web3::keys::pink::KeyPair::derive_keypair(b"executor")
                    .private_key(),
            }
        }

        pub fn get_chain(&self, name: String) -> Option<Chain> {
            let bytes = self.raw_graph.clone();
            let mut bytes = bytes.as_ref();
            let graph = Graph::decode(&mut bytes).unwrap();
            graph.get_chain(name)
        }

        #[ink(message)]
        pub fn config(
            &mut self,
            registry: AccountId,
            rollup_endpoint: String,
            pallet_id: Option<u8>,
        ) -> Result<()> {
            self.ensure_owner()?;
            self.config = Some(Config {
                registry: RegistryRef::from_account_id(registry),
                pallet_id,
            });

            // Read graph data from registry contract
            self.sync_graph()?;

            // If we don't give the pallet_id, skip rollup configuration
            if pallet_id.is_none() {
                return Ok(());
            }

            let contract_id = self.env().account_id();
            // Check if the rollup is initialized properly
            let actual_owner = get_name_owner(&rollup_endpoint, &contract_id)
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
                    &rollup_endpoint,
                    self.config.clone().unwrap().pallet_id.unwrap(),
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
        pub fn setup_worker_accounts(&mut self) -> Result<()> {
            let config = self.ensure_configured()?;
            // Get rpc info from registry
            let chain = self
                .get_chain("Khala".to_string())
                .map(Ok)
                .unwrap_or(Err(Error::ChainNotFound))?;
            let contract_id = self.env().account_id();
            let mut client = SubstrateRollupClient::new(
                &chain.endpoint,
                config.pallet_id.unwrap(),
                &contract_id,
            )
            .log_err("failed to create rollup client")
            .or(Err(Error::FailedToCreateClient))?;

            // Setup worker accounts if it hasn't been set yet.
            if OnchainAccounts::lookup_free_accounts(&mut client).len() == 0 {
                OnchainAccounts::set_worker_accounts(
                    &mut client,
                    self.worker_accounts
                        .clone()
                        .into_iter()
                        .map(|account| account.account32.clone())
                        .collect(),
                );
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
            }

            Ok(())
        }

        #[ink(message)]
        pub fn run(&self, running_type: RunningType) -> Result<()> {
            let config = self.ensure_configured()?;
            // Get rpc info from registry
            let chain = self
                .get_chain("Khala".to_string())
                .map(Ok)
                .unwrap_or(Err(Error::ChainNotFound))?;
            let contract_id = self.env().account_id();
            let mut client = SubstrateRollupClient::new(
                &chain.endpoint,
                config.pallet_id.unwrap(),
                &contract_id,
            )
            .log_err("failed to create rollup client")
            .or(Err(Error::FailedToCreateClient))?;

            match running_type {
                RunningType::Fetch(source_chain) => self.fetch_task(&mut client, source_chain)?,
                RunningType::Execute => self.execute_task(&mut client)?,
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

        #[ink(message)]
        pub fn sync_graph(&mut self) -> Result<()> {
            let config = self.ensure_configured()?;
            let registry_graph = config.registry.clone().get_graph();
            let mut local_graph: Graph = Graph::default();

            {
                let len = registry_graph.chains.len();
                let mut arr: Vec<Chain> = Vec::new();
                arr.resize(len + 1, Chain::default());
                for chain in registry_graph.chains {
                    let item: Chain = Chain {
                        id: chain.id,
                        name: chain.name,
                        endpoint: chain.endpoint,
                        chain_type: {
                            match chain.chain_type {
                                // 0 => ChainType::Unknown,
                                1 => ChainType::Evm,
                                2 => ChainType::Sub,
                                _ => panic!("Unsupported chain!"),
                            }
                        },
                    };
                    arr[chain.id as usize] = item;
                }
                local_graph.chains = arr;
            }

            {
                let len = registry_graph.assets.len();
                let mut arr: Vec<Asset> = Vec::new();
                arr.resize(len + 1, Asset::default());
                for asset in registry_graph.assets {
                    let item = Asset {
                        id: asset.id,
                        symbol: asset.symbol,
                        name: asset.name,
                        // beware the special treatment for locations!
                        // reason:
                        //  ink! treats any string that starts with a 0x prefix as a hex string,
                        //  if `location` starts with 0x then we will get an unreadable character string here,
                        //  a workaround is to encode the location
                        //      (and anything that is possibly a string prefixed with 0x) by hex-ing it,
                        //      before putting it in the ink! storage;
                        //  now in time of use, we decode the location by hex::decode()
                        location: hex::decode(asset.location).unwrap(),
                        decimals: asset.decimals,
                        chain_id: asset.chain_id,
                    };
                    arr[asset.id as usize] = item;
                }
                local_graph.assets = arr;
            }

            {
                let len = registry_graph.dexs.len();
                let mut arr = Vec::new();
                arr.resize(len + 1, Dex::default());
                for dex in registry_graph.dexs {
                    let item = Dex {
                        id: dex.id,
                        name: dex.name,
                        chain_id: dex.chain_id,
                    };
                    arr[dex.id as usize] = item;
                }
                local_graph.dexs = arr;
            }

            {
                let len = registry_graph.dex_indexers.len();
                let mut arr = Vec::new();
                arr.resize(len + 1, DexIndexer::default());
                for indexer in registry_graph.dex_indexers {
                    let item = DexIndexer {
                        id: indexer.id,
                        url: indexer.url,
                        dex_id: indexer.dex_id,
                    };
                    arr[indexer.id as usize] = item;
                }
                local_graph.dex_indexers = arr;
            }

            {
                let len = registry_graph.dex_pairs.len();
                let mut arr = Vec::new();
                arr.resize(len + 1, DexPair::default());
                for pair in registry_graph.dex_pairs {
                    let item = DexPair {
                        id: pair.id,
                        asset0_id: pair.asset0_id,
                        asset1_id: pair.asset1_id,
                        dex_id: pair.dex_id,
                        // caveat, for now we have two kinds of pair_id:
                        //  1. 0x1234...23
                        //  2. lp:$TOEKN1/$TOKEN2
                        // we need to hexify the first kind to get around the ink! string treatment,
                        // to that end, we hexify all kinds of pair_id
                        pair_id: hex::decode(pair.pair_id).unwrap(),
                    };
                    arr[pair.id as usize] = item;
                }
                local_graph.dex_pairs = arr;
            }

            {
                let len = registry_graph.bridges.len();
                let mut arr = Vec::new();
                arr.resize(len + 1, Bridge::default());
                for bridge in registry_graph.bridges {
                    let item = Bridge {
                        id: bridge.id,
                        name: bridge.name,
                        location: hex::decode(bridge.location).unwrap(),
                    };
                    arr[bridge.id as usize] = item;
                }
                local_graph.bridges = arr;
            }

            {
                let len = registry_graph.bridge_pairs.len();
                let mut arr = Vec::new();
                arr.resize(len + 1, BridgePair::default());
                for pair in registry_graph.bridge_pairs {
                    let item = BridgePair {
                        id: pair.id,
                        asset0_id: pair.asset0_id,
                        asset1_id: pair.asset1_id,
                        bridge_id: pair.bridge_id,
                    };
                    arr[pair.id as usize] = item;
                }
                local_graph.bridge_pairs = arr;
            }

            self.raw_graph = local_graph.encode();

            Ok(())
        }

        /// For cross-contract call test
        #[ink(message)]
        pub fn get_graph(&self) -> Result<RegistryGraph> {
            let config = self.ensure_configured()?;
            let graph = config.registry.clone().get_graph();
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

        /// Return worker accounts information
        #[ink(message)]
        pub fn get_free_worker_account(&self) -> Result<Vec<[u8; 32]>> {
            let config = self.ensure_configured()?;
            // Get rpc info from registry
            let chain = self
                .get_chain("Khala".to_string())
                .map(Ok)
                .unwrap_or(Err(Error::ChainNotFound))?;
            let contract_id = self.env().account_id();
            let mut client = SubstrateRollupClient::new(
                &chain.endpoint,
                config.pallet_id.unwrap(),
                &contract_id,
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
        ) -> Result<()> {
            // Fetch actived task that completed initial confirmation from specific chain that belong to current worker,
            // and append them to runing tasks
            let mut actived_task = ActivedTaskFetcher::new(
                self.get_chain(source_chain).unwrap(),
                AccountInfo::from(self.executor_account),
            )
            .fetch_task()
            .map_err(|_| Error::FailedToFetchTask)?;

            // Initialize task, and save it to on-chain storage
            actived_task.init(
                &Context {
                    // Don't need signer here
                    signer: [0; 32],
                    graph: {
                        let bytes = self.raw_graph.clone();
                        let mut bytes = bytes.as_ref();
                        Graph::decode(&mut bytes).unwrap()
                    },
                    worker_accounts: self.worker_accounts.clone(),
                    bridge_executors: vec![],
                    dex_executors: vec![],
                },
                client,
            );

            Ok(())
        }

        /// Execute tasks from all supported blockchains. This is a query operation
        /// that scheduler invokes periodically.
        pub fn execute_task(&self, client: &mut SubstrateRollupClient) -> Result<()> {
            for id in OnchainTasks::lookup_pending_tasks(client).iter() {
                // Get task saved in local cache, if not exist in local, try recover from on-chain storage
                let mut task = TaskCache::get_task(id)
                    .or_else(|| {
                        if let Some(mut onchain_task) = OnchainTasks::lookup_task(client, id) {
                            onchain_task.sync(client);
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
                    graph: {
                        let bytes = self.raw_graph.clone();
                        let mut bytes = bytes.as_ref();
                        Graph::decode(&mut bytes).unwrap()
                    },
                    worker_accounts: self.worker_accounts.clone(),
                    bridge_executors: self.create_bridge_executors()?,
                    dex_executors: self.create_dex_executors()?,
                }) {
                    Ok(TaskStatus::Completed) => {
                        // Remove task from blockchain and recycle worker account
                        task.destroy(client);
                        // If task already delete from rollup storage, delete it from local cache
                        if OnchainTasks::lookup_task(client, id).is_none() {
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
                .map(|idx| self.worker_prv_keys[idx])
        }

        #[allow(clippy::type_complexity)]
        fn create_bridge_executors(
            &self,
        ) -> Result<Vec<((String, String), Box<dyn BridgeExecutor>)>> {
            // let config = self.ensure_configured()?;
            let mut bridge_executors: Vec<((String, String), Box<dyn BridgeExecutor>)> = vec![];
            let ethereum = self
                .get_chain(String::from("Ethereum"))
                .map(Ok)
                .unwrap_or(Err(Error::ChainNotFound))?;
            let phala = self
                .get_chain(String::from("Phala"))
                .map(Ok)
                .unwrap_or(Err(Error::ChainNotFound))?;

            // Ethereum -> Phala: ChainBridgeEvm2Phala
            let chainbridge_on_ethereum: H160 =
                hex!("056c0e37d026f9639313c281250ca932c9dbe921").into();
            // PHA ChainBridge resource id on Khala
            let pha_rid: H256 =
                hex!("00e6dfb61a2fb903df487c401663825643bb825d41695e63df8af6162ab145a6").into();
            // PHA contract address on Ethereum
            let pha_contract: H160 = hex!("6c5bA91642F10282b576d91922Ae6448C9d52f4E").into();
            bridge_executors.push((
                (String::from("Ethereum"), String::from("Phala")),
                Box::new(ChainBridgeEvm2Phala::new(
                    &ethereum.endpoint,
                    chainbridge_on_ethereum,
                    vec![(pha_contract, pha_rid.into())],
                )),
            ));

            // Phala -> Ethereum: ChainBridgePhala2Evm
            bridge_executors.push((
                (String::from("Phala"), String::from("Ethereum")),
                Box::new(ChainBridgePhala2Evm::new(
                    // ChainId of Ethereum under the ChainBridge protocol
                    0,
                    &phala.endpoint,
                )),
            ));

            Ok(bridge_executors)
        }

        fn create_dex_executors(&self) -> Result<Vec<(String, Box<dyn DexExecutor>)>> {
            // let config = self.ensure_configured()?;
            let mut dex_executors: Vec<(String, Box<dyn DexExecutor>)> = vec![];
            let ethereum = self
                .get_chain(String::from("Ethereum"))
                .map(Ok)
                .unwrap_or(Err(Error::ChainNotFound))?;

            dex_executors.push((
                String::from("Phala"),
                Box::new(UniswapV2Executor::new(
                    &ethereum.endpoint,
                    // UniswapV2 router address on Ethereum
                    hex!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D").into(),
                )),
            ));

            Ok(dex_executors)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        // use dotenv::dotenv;
        use index_registry::{Chain as RegistryChain, Graph, Registry};
        use ink::ToAccountId;
        use ink_lang as ink;
        use phala_pallet_common::WrapSlice;
        // use pink_extension::PinkEnvironment;
        use xcm::latest::{prelude::*, MultiLocation};

        fn deploy_registry() -> RegistryRef {
            // Register contracts
            let hash = ink_env::Hash::try_from([10u8; 32]).unwrap();
            ink_env::test::register_contract::<Registry>(hash.as_ref());

            // Deploy Registry
            RegistryRef::new()
                .code_hash(hash)
                .endowment(0)
                .salt_bytes([0u8; 0])
                .instantiate()
                .expect("failed to deploy Registry")
        }

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
            ExecutorRef::new()
                .code_hash(hash)
                .endowment(0)
                .salt_bytes([0u8; 0])
                .instantiate()
                .expect("failed to deploy Executor")
        }

        #[ink::test]
        fn crosscontract_call_should_work() {
            pink_extension_runtime::mock_ext::mock_all_ext();

            let registry = deploy_registry();
            let mut executor = deploy_executor();
            assert_eq!(
                executor.config(registry.to_account_id(), String::from("khala rpc"), None),
                Ok(())
            );

            // Make cross contract call from executor
            assert_eq!(
                executor.get_graph().unwrap(),
                RegistryGraph {
                    chains: vec![],
                    assets: vec![],
                    dexs: vec![],
                    dex_pairs: vec![],
                    dex_indexers: vec![],
                    bridges: vec![],
                    bridge_pairs: vec![],
                }
            )
        }

        #[ink::test]
        fn rollup_should_work() {
            pink_extension_runtime::mock_ext::mock_all_ext();
            // let registry = deploy_registry();
            let mut _executor = deploy_executor();
            // Initial rollup
            // Comment because we can not test it in CI so far
            // assert_eq!(
            //     executor.config(
            //         registry.to_account_id(),
            //         String::from("http://127.0.0.1:39933"),
            //         Some(100)
            //     ),
            //     Ok(())
            // );
        }

        #[ink::test]
        fn setup_worker_accounts_should_work() {
            pink_extension_runtime::mock_ext::mock_all_ext();

            let mut registry = deploy_registry();
            assert_eq!(
                registry.set_graph(Graph {
                    chains: vec![RegistryChain {
                        id: 1,
                        name: String::from("Khala"),
                        endpoint: String::from("http://127.0.0.1:39933"),
                        // 2 for Sub
                        chain_type: 2,
                    }],
                    assets: vec![],
                    dexs: vec![],
                    dex_pairs: vec![],
                    dex_indexers: vec![],
                    bridges: vec![],
                    bridge_pairs: vec![],
                }),
                Ok(())
            );
            let mut _executor = deploy_executor();
            // Initial rollup
            // Comment because we can not test it in CI so far
            // assert_eq!(
            //     executor.config(
            //         registry.to_account_id(),
            //         String::from("http://127.0.0.1:39933"),
            //         Some(100)
            //     ),
            //     Ok(())
            // );
            // assert_eq!(executor.setup_worker_accounts(), Ok(()));
            // let onchain_free_accounts = executor.get_free_worker_account().unwrap();
            // let local_worker_accounts: Vec<[u8; 32]> = executor
            //     .get_worker_account()
            //     .into_iter()
            //     .map(|account| account.account32.clone())
            //     .collect();
            // assert_eq!(onchain_free_accounts, local_worker_accounts);
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
