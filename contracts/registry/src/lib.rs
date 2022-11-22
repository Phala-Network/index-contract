#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use ink_lang as ink;

mod bridge;
mod chain;
mod chain_store;
mod dex;
mod types;

#[allow(clippy::large_enum_variant)]
#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_registry {
    use crate::bridge::{AssetPair, Bridge};
    use crate::chain::Chain;
    use crate::dex::{Dex, DexPair};
    use crate::types::Error;
    use crate::types::*;
    use alloc::{string::String, vec::Vec};
    use index::ensure;
    use ink_storage::traits::SpreadAllocate;
    use ink_storage::Mapping;

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Registry {
        pub admin: AccountId,

        pub supported_chains: Vec<Vec<u8>>,
        /// The registered chains. [chain_name, entity]
        pub chains: Mapping<Vec<u8>, Chain>,

        pub supported_bridges: Vec<String>,
        pub bridges: Mapping<String, Bridge>,

        pub supported_dexs: Vec<String>,
        pub dexs: Mapping<String, Dex>,
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

    /// Event emitted when bridge registered.
    /// args: [bridge_name]
    #[ink(event)]
    pub struct BridgeRegistered {
        #[ink(topic)]
        name: String,
    }

    /// Event emitted when bridge unregistered.
    /// args: [bridge_name]
    #[ink(event)]
    pub struct BridgeUnregistered {
        #[ink(topic)]
        name: String,
    }

    /// Event emitted when bridge asset registered.
    /// args: [bridge_name, asset_pair]
    #[ink(event)]
    pub struct BridgeAssetRegistered {
        #[ink(topic)]
        name: String,
        pair: AssetPair,
    }

    /// Event emitted when bridge asset unregistered.
    /// args: [bridge_name, asset_pair]
    #[ink(event)]
    pub struct BridgeAssetUnregistered {
        #[ink(topic)]
        name: String,
        pair: AssetPair,
    }

    /// Event emitted when bridge registered.
    /// args: [dex_id, dex_name]
    #[ink(event)]
    pub struct DexRegistered {
        #[ink(topic)]
        id: Vec<u8>,
        #[ink(topic)]
        name: String,
    }

    /// Event emitted when bridge unregistered.
    /// args: [dex_id]
    #[ink(event)]
    pub struct DexUnregistered {
        #[ink(topic)]
        name: String,
    }

    /// Event emitted when dex trading pair registered.
    /// args: [dex_id, trading_pair]
    #[ink(event)]
    pub struct DexPairRegistered {
        #[ink(topic)]
        id: Vec<u8>,
        pair: DexPair,
    }

    /// Event emitted when dex trading pair unregistered.
    /// args: [dex_id, trading_pair]
    #[ink(event)]
    pub struct DexPairUnregistered {
        #[ink(topic)]
        id: Vec<u8>,
        pair: DexPair,
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

        #[ink(message)]
        pub fn register_bridge(
            &mut self,
            name: String,
            chain0: ChainInfo,
            chain1: ChainInfo,
        ) -> Result<()> {
            self.esure_admin()?;
            ensure!(
                !self.bridges.contains(&name),
                Error::BridgeAlreadyRegistered
            );
            self.bridges
                .insert(&name, &Bridge::new(name.clone(), chain0, chain1));
            let mut supported_bridges = self.supported_bridges.clone();
            supported_bridges.push(name.clone());
            self.supported_bridges = supported_bridges;
            Self::env().emit_event(BridgeRegistered { name });
            Ok(())
        }

        #[ink(message)]
        pub fn unregister_bridge(&mut self, name: String) -> Result<()> {
            self.esure_admin()?;

            ensure!(self.bridges.get(&name).is_some(), Error::BridgeNotFound);
            self.bridges.remove(&name);
            let mut supported_bridges = self.supported_bridges.clone();
            supported_bridges.retain(|x| x != &name);
            self.supported_bridges = supported_bridges;
            Self::env().emit_event(BridgeUnregistered { name });
            Ok(())
        }

        #[ink(message)]
        pub fn add_bridge_asset(&mut self, bridge_name: String, pair: AssetPair) -> Result<()> {
            self.esure_admin()?;

            let mut bridge = self
                .bridges
                .get(&bridge_name)
                .ok_or(Error::BridgeNotFound)?;
            ensure!(
                self.asset_registered(&bridge.chain0.name, &pair.asset0)
                    && self.asset_registered(&bridge.chain1.name, &pair.asset1),
                Error::AssetNotFound
            );
            bridge.register(pair.clone())?;
            // Insert back
            self.bridges.insert(&bridge_name, &bridge);
            Self::env().emit_event(BridgeAssetRegistered {
                name: bridge_name,
                pair,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn remove_bridge_asset(&mut self, bridge_name: String, pair: AssetPair) -> Result<()> {
            self.esure_admin()?;

            let mut bridge = self
                .bridges
                .get(&bridge_name)
                .ok_or(Error::BridgeNotFound)?;
            bridge.unregister(pair.clone())?;
            // Insert back
            self.bridges.insert(&bridge_name, &bridge);
            Self::env().emit_event(BridgeAssetUnregistered {
                name: bridge_name,
                pair,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn register_dex(&mut self, name: String, id: Vec<u8>, chain: ChainInfo) -> Result<()> {
            self.esure_admin()?;
            ensure!(!self.dexs.contains(&name), Error::DexAlreadyRegistered);
            self.dexs
                .insert(&name, &Dex::new(name.clone(), id.clone(), chain));
            let mut supported_dexs = self.supported_dexs.clone();
            supported_dexs.push(name.clone());
            self.supported_dexs = supported_dexs;
            Self::env().emit_event(DexRegistered { id, name });
            Ok(())
        }

        #[ink(message)]
        pub fn unregister_dex(&mut self, name: String) -> Result<()> {
            self.esure_admin()?;

            ensure!(self.dexs.get(&name).is_some(), Error::DexNotFound);
            self.dexs.remove(&name);
            let mut supported_dexs = self.supported_dexs.clone();
            supported_dexs.retain(|x| x != &name);
            self.supported_dexs = supported_dexs;
            Self::env().emit_event(DexUnregistered { name });
            Ok(())
        }

        #[ink(message)]
        pub fn add_dex_pair(&mut self, dex_name: String, pair: DexPair) -> Result<()> {
            self.esure_admin()?;

            let mut dex = self.dexs.get(&dex_name).ok_or(Error::DexNotFound)?;
            ensure!(
                self.asset_registered(&dex.chain.name, &pair.asset0)
                    && self.asset_registered(&dex.chain.name, &pair.asset1),
                Error::AssetNotFound
            );
            dex.register(pair.clone())?;
            // Insert back
            self.dexs.insert(&dex_name, &dex);
            Self::env().emit_event(DexPairRegistered { id: dex.id, pair });
            Ok(())
        }

        #[ink(message)]
        pub fn remove_dex_pair(&mut self, dex_name: String, pair: DexPair) -> Result<()> {
            self.esure_admin()?;

            let mut dex = self.dexs.get(&dex_name).ok_or(Error::DexNotFound)?;
            dex.unregister(pair.clone())?;
            // Insert back
            self.dexs.insert(&dex_name, &dex);
            Self::env().emit_event(DexPairUnregistered { id: dex.id, pair });
            Ok(())
        }

        /// Return true if asset has been registered on the specific chain
        fn asset_registered(&self, chain_name: &Vec<u8>, asset: &AssetInfo) -> bool {
            if let Some(chain_entity) = self.chains.get(chain_name) {
                chain_entity
                    .lookup_by_location(asset.location.clone())
                    .is_some()
            } else {
                false
            }
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
        use crate::chain::EvmBalance;
        use dotenv::dotenv;
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

        #[ink::test]
        fn bridge_registry_should_works() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let ethereum = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            let phala = ChainInfo {
                name: b"Phala".to_vec(),
                chain_type: ChainType::Sub,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            let pha_on_ethereum = AssetInfo {
                name: b"Phala Token".to_vec(),
                symbol: b"PHA".to_vec(),
                decimals: 18,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            let pha_on_phala = AssetInfo {
                name: b"Phala Token".to_vec(),
                symbol: b"PHA".to_vec(),
                decimals: 12,
                location: b"Somewhere on Phala".to_vec(),
            };
            let ethereum2phala_pha_pair = AssetPair {
                asset0: pha_on_ethereum.clone(),
                asset1: pha_on_phala.clone(),
            };

            assert_eq!(registry.register_chain(ethereum.clone()), Ok(()));
            assert_eq!(
                registry.register_asset(ethereum.name.clone(), pha_on_ethereum),
                Ok(())
            );
            assert_eq!(registry.register_chain(phala.clone()), Ok(()));
            assert_eq!(
                registry.register_asset(phala.name.clone(), pha_on_phala),
                Ok(())
            );
            assert_eq!(
                registry.register_bridge(
                    "Bridge_Phala2Ethereum".to_string(),
                    phala.clone(),
                    ethereum.clone()
                ),
                Ok(())
            );
            assert_eq!(
                registry.register_bridge("Bridge_Ethereum2Phala".to_string(), ethereum, phala),
                Ok(())
            );
            assert_eq!(
                registry.supported_bridges,
                vec![
                    "Bridge_Phala2Ethereum".to_string(),
                    "Bridge_Ethereum2Phala".to_string()
                ]
            );
            assert_eq!(
                registry.unregister_bridge("Bridge_Phala2Ethereum".to_string()),
                Ok(())
            );
            assert_eq!(
                registry.supported_bridges,
                vec!["Bridge_Ethereum2Phala".to_string()]
            );

            assert_eq!(
                registry.add_bridge_asset(
                    "Bridge_Ethereum2Phala".to_string(),
                    ethereum2phala_pha_pair.clone()
                ),
                Ok(())
            );
            assert_eq!(
                registry
                    .bridges
                    .get(&"Bridge_Ethereum2Phala".to_string())
                    .unwrap()
                    .assets,
                vec![ethereum2phala_pha_pair.clone()]
            );
            assert_eq!(
                registry.remove_bridge_asset(
                    "Bridge_Ethereum2Phala".to_string(),
                    ethereum2phala_pha_pair
                ),
                Ok(())
            );
            assert_eq!(
                registry
                    .bridges
                    .get(&"Bridge_Ethereum2Phala".to_string())
                    .unwrap()
                    .assets,
                vec![]
            );
        }

        #[ink::test]
        fn dex_registry_should_works() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut registry = Registry::new();

            let ethereum = ChainInfo {
                name: b"Ethereum".to_vec(),
                chain_type: ChainType::Evm,
                native: None,
                stable: None,
                endpoint: b"endpoint".to_vec(),
                network: None,
            };
            let pha = AssetInfo {
                name: b"Phala Token".to_vec(),
                symbol: b"PHA".to_vec(),
                decimals: 18,
                location: b"Somewhere on Ethereum1".to_vec(),
            };
            let weth = AssetInfo {
                name: b"Wrap Ether".to_vec(),
                symbol: b"WETH".to_vec(),
                decimals: 18,
                location: b"Somewhere on Ethereum2".to_vec(),
            };

            let pha_weth_pair = DexPair {
                id: b"pair_address".to_vec(),
                asset0: pha.clone(),
                asset1: weth.clone(),
                swap_fee: Some(0),
                dev_fee: Some(0),
            };

            assert_eq!(registry.register_chain(ethereum.clone()), Ok(()));
            assert_eq!(
                registry.register_asset(ethereum.name.clone(), pha.clone()),
                Ok(())
            );
            assert_eq!(
                registry.register_asset(ethereum.name.clone(), weth.clone()),
                Ok(())
            );
            assert_eq!(
                registry.register_dex(
                    "UniswapV2".to_string(),
                    b"UniswapV2 factory".to_vec(),
                    ethereum.clone()
                ),
                Ok(())
            );
            assert_eq!(
                registry.register_dex(
                    "SushiSwap".to_string(),
                    b"SushiSwap factory".to_vec(),
                    ethereum.clone()
                ),
                Ok(())
            );
            assert_eq!(
                registry.supported_dexs,
                vec!["UniswapV2".to_string(), "SushiSwap".to_string()]
            );
            assert_eq!(registry.unregister_dex("SushiSwap".to_string()), Ok(()));
            assert_eq!(registry.supported_dexs, vec!["UniswapV2".to_string()]);
            assert_eq!(
                registry.add_dex_pair("UniswapV2".to_string(), pha_weth_pair.clone()),
                Ok(())
            );
            assert_eq!(
                registry.dexs.get(&"UniswapV2".to_string()).unwrap().pairs,
                vec![pha_weth_pair.clone()]
            );
            assert_eq!(
                registry.remove_dex_pair("UniswapV2".to_string(), pha_weth_pair),
                Ok(())
            );
            assert_eq!(
                registry.dexs.get(&"UniswapV2".to_string()).unwrap().pairs,
                vec![]
            );
        }
    }
}
