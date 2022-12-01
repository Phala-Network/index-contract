#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use ink_lang as ink;

mod task;

#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_executor {
    use alloc::{vec, vec::Vec};
    use scale::{Decode, Encode};
    // use index::{ensure, prelude::*};
    use index::utils::ToArray;
    use index_registry::{types::Graph, RegistryRef};
    use ink_env::call::FromAccountId;
    use pink_extension::chain_extension::{signing, SigType};

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        ReadCacheFailed,
        WriteCacheFailed,
        DecodeCacheFailed,
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
        pub admin: AccountId,
        pub registry: Option<RegistryRef>,
        pub worker_accounts: Vec<[u8; 32]>,
        pub executor_account: [u8; 32],
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
                registry: None,
                worker_accounts,
                executor_account: pink_web3::keys::pink::KeyPair::derive_keypair(b"executor")
                    .private_key(),
            }
        }

        /// Create an Executor entity
        #[ink(message)]
        pub fn set_registry(&mut self, registry: AccountId) -> Result<()> {
            self.registry = Some(RegistryRef::from_account_id(registry));
            Ok(())
        }

        /// For cross-contract call test
        #[ink(message)]
        pub fn get_graph(&self) -> Result<Graph> {
            let graph = self.registry.clone().unwrap().get_graph().unwrap();
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
        use pink_extension::PinkEnvironment;

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
    }
}
