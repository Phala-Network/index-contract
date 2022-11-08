#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use ink_lang as ink;

#[allow(clippy::large_enum_variant)]
#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_registry {
    use alloc::vec::Vec;
    use index::ensure;
    use index::prelude::*;
    use index::registry::chain::Chain;
    use ink_storage::traits::SpreadAllocate;
    use ink_storage::Mapping;

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Registry {
        pub admin: AccountId,
        /// The registered chains. [chain_name, entity]
        pub chains: Mapping<Vec<u8>, Chain>,
    }

    impl Default for Registry {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Event emitted when chain registered.
    #[ink(event)]
    pub struct ChainRegistered {
        #[ink(topic)]
        chain: ChainInfo,
    }

    /// Event emitted when chain unregistered.
    #[ink(event)]
    pub struct ChainUnregistered {
        #[ink(topic)]
        chain: Vec<u8>,
    }

    /// Event emitted when native asset set.
    #[ink(event)]
    pub struct ChainNativeSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: AssetInfo,
    }

    /// Event emitted when stable asset set.
    #[ink(event)]
    pub struct ChainStableSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: AssetInfo,
    }

    /// Event emitted when RPC endpoint asset set.
    #[ink(event)]
    pub struct ChainEndpointSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        endpoint: Vec<u8>,
    }

    /// Event emitted when asset registered.
    #[ink(event)]
    pub struct AssetRegistered {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: AssetInfo,
    }

    /// Event emitted when asset unregistered.
    #[ink(event)]
    pub struct AssetUnregistered {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: AssetInfo,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    impl Registry {
        #[ink(constructor)]
        /// Create an Ethereum entity
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(|this: &mut Self| {
                this.admin = Self::env().caller();
            })
        }

        /// Register a chain
        /// Authorized method, only the contract owner can call
        #[ink(message)]
        pub fn register_chain(&mut self, info: ChainInfo) -> Result<()> {
            self.esure_admin()?;
            ensure!(
                !self.chains.contains(&info.name),
                Error::ChainAlreadyRegistered
            );
            self.chains.insert(&info.name, &Chain::new(info.clone()));
            Self::env().emit_event(ChainRegistered { chain: info });
            Ok(())
        }

        /// Unregister a chain
        /// Authorized method, only the contract owner can call
        #[ink(message)]
        pub fn unregister_chain(&mut self, name: Vec<u8>) -> Result<()> {
            self.esure_admin()?;

            ensure!(self.chains.get(&name).is_some(), Error::ChainNotFound);
            self.chains.remove(&name);
            Self::env().emit_event(ChainUnregistered { chain: name });
            Ok(())
        }

        /// Register an asset for a chain
        /// Authorized method, only the contract owner can call
        #[ink(message)]
        pub fn register_asset(&mut self, chain: Vec<u8>, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;

            let mut chain_entity = self.chains.get(&chain).ok_or(Error::ChainNotFound)?;
            chain_entity.register(asset.clone())?;
            // Insert back
            self.chains.insert(&chain, &chain_entity);
            Self::env().emit_event(AssetRegistered { chain, asset });
            Ok(())
        }

        /// Unregister an asset from a chain
        /// Authorized method, only the contract owner can call
        #[ink(message)]
        pub fn unregister_asset(&mut self, chain: Vec<u8>, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;

            let mut chain_entity = self.chains.get(&chain).ok_or(Error::ChainNotFound)?;
            chain_entity.unregister(asset.clone())?;
            // Insert back
            self.chains.insert(&chain, &chain_entity);
            Self::env().emit_event(AssetUnregistered { chain, asset });
            Ok(())
        }

        /// Set native asset
        /// Authorized method, only the contract owner can call
        #[ink(message)]
        pub fn set_chain_native(&mut self, chain: Vec<u8>, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;

            let mut chain_entity = self.chains.get(&chain).ok_or(Error::ChainNotFound)?;
            chain_entity.set_native(asset.clone());
            // Insert back
            self.chains.insert(&chain, &chain_entity);
            Self::env().emit_event(ChainNativeSet { chain, asset });
            Ok(())
        }

        /// Set native asset
        /// Authorized method, only the contract owner can call
        #[ink(message)]
        pub fn set_chain_stable(&mut self, chain: Vec<u8>, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;

            let mut chain_entity = self.chains.get(&chain).ok_or(Error::ChainNotFound)?;
            chain_entity.set_stable(asset.clone());
            // Insert back
            self.chains.insert(&chain, &chain_entity);
            Self::env().emit_event(ChainStableSet { chain, asset });
            Ok(())
        }

        /// Set RPC endpoint
        /// Authorized method, only the contract owner can call
        #[ink(message)]
        pub fn set_chain_endpoint(&mut self, chain: Vec<u8>, endpoint: Vec<u8>) -> Result<()> {
            self.esure_admin()?;

            let mut chain_entity = self.chains.get(&chain).ok_or(Error::ChainNotFound)?;
            chain_entity.set_endpoint(endpoint.clone());
            // Insert back
            self.chains.insert(&chain, &chain_entity);
            Self::env().emit_event(ChainEndpointSet { chain, endpoint });
            Ok(())
        }

        /// Returns error if caller is not admin
        fn esure_admin(&self) -> Result<()> {
            let caller = self.env().caller();
            if self.admin != caller {
                return Err(Error::BadOrigin);
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use dotenv::dotenv;
        use index::registry::chain::EvmBalance;
        use ink_lang as ink;
        use phala_pallet_common::WrapSlice;
        use pink_extension::PinkEnvironment;
        use scale::Encode;
        use xcm::latest::{prelude::*, MultiLocation};

        type Event = <Registry as ink::reflect::ContractEventBase>::Type;

        fn default_accounts() -> ink_env::test::DefaultAccounts<PinkEnvironment> {
            ink_env::test::default_accounts::<PinkEnvironment>()
        }

        fn set_caller(sender: AccountId) {
            ink_env::test::set_caller::<PinkEnvironment>(sender);
        }

        fn assert_events(mut expected: Vec<Event>) {
            let mut actual: Vec<ink_env::test::EmittedEvent> =
                ink_env::test::recorded_events().collect();

            assert_eq!(actual.len(), expected.len(), "Event count don't match");
            expected.reverse();

            for evt in expected {
                let next = actual.pop().expect("event expected");
                // Compare event data
                assert_eq!(
                    next.data,
                    <Event as Encode>::encode(&evt),
                    "Event data don't match"
                );
            }
        }

        #[ink::test]
        fn test_default_works() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let registry = Registry::new();
            assert_eq!(registry.admin, accounts.alice);
        }

        #[ink::test]
        fn test_register_chain_should_works() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let evmchain_info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            let subchain_info = ChainInfo {
                name: b"Phala".to_vec(),
                chain_type: ChainType::Sub,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(evmchain_info.clone()), Ok(()));
            assert_eq!(registry.register_chain(subchain_info.clone()), Ok(()));
            assert_events(vec![
                ChainRegistered {
                    chain: evmchain_info,
                }
                .into(),
                ChainRegistered {
                    chain: subchain_info,
                }
                .into(),
            ]);
        }

        #[ink::test]
        fn test_dumplicated_register_chain_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            assert_eq!(
                registry.register_chain(info),
                Err(Error::ChainAlreadyRegistered)
            );
        }

        #[ink::test]
        fn test_unregister_chain_should_works() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            assert_eq!(registry.unregister_chain(info.name.clone()), Ok(()));
            assert_events(vec![
                ChainRegistered {
                    chain: info.clone(),
                }
                .into(),
                ChainUnregistered { chain: info.name }.into(),
            ]);
        }

        #[ink::test]
        fn test_set_native_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));

            let weth = AssetInfo {
                name: b"Wrap Ether".to_vec(),
                symbol: b"WETH".to_vec(),
                decimals: 18,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(
                registry.set_chain_native(info.name.clone(), weth.clone()),
                Ok(())
            );
            assert_events(vec![
                ChainRegistered {
                    chain: info.clone(),
                }
                .into(),
                ChainNativeSet {
                    chain: b"Ethereum".to_vec(),
                    asset: weth.clone(),
                }
                .into(),
            ]);
            let chain = registry.chains.get(info.name.clone()).unwrap();
            assert_eq!(chain.get_info().native, Some(weth));
        }

        #[ink::test]
        fn test_set_native_without_permisssion_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            let weth = AssetInfo {
                name: b"Wrap Ether".to_vec(),
                symbol: b"WETH".to_vec(),
                decimals: 18,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            set_caller(accounts.bob);
            assert_eq!(
                registry.set_chain_native(info.name, weth),
                Err(Error::BadOrigin)
            );
        }

        #[ink::test]
        fn test_set_stable_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));

            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(
                registry.set_chain_stable(info.name.clone(), usdc.clone()),
                Ok(())
            );
            assert_events(vec![
                ChainRegistered {
                    chain: info.clone(),
                }
                .into(),
                ChainStableSet {
                    chain: b"Ethereum".to_vec(),
                    asset: usdc.clone(),
                }
                .into(),
            ]);
            let chain = registry.chains.get(info.name.clone()).unwrap();
            assert_eq!(chain.get_info().stable, Some(usdc));
        }

        #[ink::test]
        fn test_set_stable_without_permisssion_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            set_caller(accounts.bob);
            assert_eq!(
                registry.set_chain_native(info.name, usdc),
                Err(Error::BadOrigin)
            );
        }

        #[ink::test]
        fn test_set_endpoint_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            assert_eq!(
                registry.set_chain_endpoint(info.name.clone(), b"new endpoint".to_vec()),
                Ok(())
            );

            assert_events(vec![
                ChainRegistered {
                    chain: info.clone(),
                }
                .into(),
                ChainEndpointSet {
                    chain: b"Ethereum".to_vec(),
                    endpoint: b"new endpoint".to_vec(),
                }
                .into(),
            ]);
            let chain = registry.chains.get(info.name.clone()).unwrap();
            assert_eq!(chain.get_info().endpoint, b"new endpoint".to_vec());
        }

        #[ink::test]
        fn test_set_endpoint_without_permisssion_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            set_caller(accounts.bob);
            assert_eq!(
                registry.set_chain_endpoint(info.name, b"new endpoint".to_vec()),
                Err(Error::BadOrigin)
            );
        }

        #[ink::test]
        fn test_register_asset_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));

            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(
                registry.register_asset(b"Ethereum".to_vec(), usdc.clone()),
                Ok(())
            );
            assert_events(vec![
                ChainRegistered {
                    chain: info.clone(),
                }
                .into(),
                AssetRegistered {
                    chain: b"Ethereum".to_vec(),
                    asset: usdc,
                }
                .into(),
            ]);
        }

        #[ink::test]
        fn test_duplicated_register_asset_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(
                registry.register_asset(info.name.clone(), usdc.clone()),
                Ok(())
            );
            assert_eq!(
                registry.register_asset(info.name, usdc),
                Err(Error::AssetAlreadyRegistered)
            );
        }

        #[ink::test]
        fn test_register_asset_without_permission_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            set_caller(accounts.bob);

            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(
                registry.register_asset(info.name, usdc),
                Err(Error::BadOrigin)
            );
        }

        #[ink::test]
        fn test_unregister_asset_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(
                registry.register_asset(info.name.clone(), usdc.clone()),
                Ok(())
            );
            assert_eq!(
                registry.unregister_asset(info.name.clone(), usdc.clone()),
                Ok(())
            );

            assert_events(vec![
                ChainRegistered {
                    chain: info.clone(),
                }
                .into(),
                AssetRegistered {
                    chain: b"Ethereum".to_vec(),
                    asset: usdc.clone(),
                }
                .into(),
                AssetUnregistered {
                    chain: b"Ethereum".to_vec(),
                    asset: usdc,
                }
                .into(),
            ]);
        }

        #[ink::test]
        fn test_unregister_unregistered_asset_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            // First time unregister
            assert_eq!(
                registry.unregister_asset(info.name.clone(), usdc.clone()),
                Err(Error::AssetNotFound)
            );
            assert_eq!(
                registry.register_asset(info.name.clone(), usdc.clone()),
                Ok(())
            );
            assert_eq!(
                registry.unregister_asset(info.name.clone(), usdc.clone()),
                Ok(())
            );
            // Second time unregister
            assert_eq!(
                registry.unregister_asset(info.name, usdc),
                Err(Error::AssetNotFound)
            );
        }

        #[ink::test]
        fn test_unregister_asset_without_permission_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            // Register by owner: alice
            assert_eq!(
                registry.register_asset(info.name.clone(), usdc.clone()),
                Ok(())
            );
            set_caller(accounts.bob);
            // Bob trying to unregister
            assert_eq!(
                registry.unregister_asset(info.name.clone(), usdc),
                Err(Error::BadOrigin)
            );
        }

        #[ink::test]
        fn test_unregister_asset_with_wrong_location_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            let wrong_usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Wrong location on Ethereum".to_vec(),
            };
            assert_eq!(registry.register_asset(info.name.clone(), usdc), Ok(()));
            assert_eq!(
                registry.unregister_asset(info.name, wrong_usdc),
                Err(Error::AssetNotFound)
            );
        }

        #[ink::test]
        fn test_query_funtions_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"https://mainnet.infura.io/v3/6d61e7957c1c489ea8141e947447405b".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"+Somewhere on Ethereum".to_vec(),
            };
            let weth = AssetInfo {
                name: b"Wrap Ether".to_vec(),
                symbol: b"WETH".to_vec(),
                decimals: 18,
                location: b"-Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(
                registry.register_asset(info.name.clone(), usdc.clone()),
                Ok(())
            );
            assert_eq!(
                registry.register_asset(info.name.clone(), weth.clone()),
                Ok(())
            );
            let chain = registry.chains.get(&info.name).unwrap();
            assert_eq!(chain.registered_assets(), vec![usdc.clone(), weth.clone()]);
            assert_eq!(chain.lookup_by_name(weth.name.clone()), Some(weth.clone()));
            assert_eq!(chain.lookup_by_name(b"Wrong Name".to_vec()), None);
            assert_eq!(
                chain.lookup_by_symbol(weth.symbol.clone()),
                Some(weth.clone())
            );
            assert_eq!(chain.lookup_by_symbol(b"Wrong Symbol".to_vec()), None);
            assert_eq!(
                chain.lookup_by_location(weth.location.clone()),
                Some(weth.clone())
            );
            assert_eq!(chain.lookup_by_location(b"Wrong Location".to_vec()), None);
            assert_eq!(registry.unregister_asset(info.name.clone(), usdc), Ok(()));
            assert_eq!(
                registry.chains.get(&info.name).unwrap().registered_assets(),
                vec![weth.clone()]
            );
            assert_eq!(registry.unregister_asset(info.name.clone(), weth), Ok(()));
            assert_eq!(
                registry.chains.get(&info.name).unwrap().registered_assets(),
                vec![]
            );
        }

        #[ink::test]
        fn test_query_balance_should_work() {
            dotenv().ok();
            use std::env;

            pink_extension_runtime::mock_ext::mock_all_ext();

            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let info = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"https://mainnet.infura.io/v3/6d61e7957c1c489ea8141e947447405b".to_vec(),
                network: None,
            };
            assert_eq!(registry.register_chain(info.clone()), Ok(()));

            let pha_location = MultiLocation::new(
                1,
                X4(
                    Parachain(2004),
                    GeneralKey(WrapSlice(b"phat").into()),
                    GeneralKey(WrapSlice(&[0; 32]).into()),
                    GeneralKey(
                        WrapSlice(&hex_literal::hex![
                            "6c5bA91642F10282b576d91922Ae6448C9d52f4E"
                        ])
                        .into(),
                    ),
                ),
            );
            let account_location = MultiLocation::new(
                1,
                X4(
                    Parachain(2004),
                    GeneralKey(WrapSlice(b"phat").into()),
                    GeneralKey(WrapSlice(&[0; 32]).into()),
                    GeneralKey(
                        WrapSlice(&hex_literal::hex![
                            "e887376a93bDa91ed66D814528D7aeEfe59990a5"
                        ])
                        .into(),
                    ),
                ),
            );
            let pha = AssetInfo {
                name: b"Phala Token".to_vec(),
                symbol: b"PHA".to_vec(),
                decimals: 18,
                location: pha_location.clone().encode(),
            };

            assert_eq!(
                registry.register_asset(info.name.clone(), pha.clone()),
                Ok(())
            );
            let chain = registry.chains.get(&info.name).unwrap();

            // If not equal, check the real balance first.
            assert_eq!(
                EvmBalance::new(chain.get_info().endpoint)
                    .balance_of(AssetId::Concrete(pha_location), account_location),
                Ok(35_000_000_000_000_000u128)
            );
        }
    }
}
