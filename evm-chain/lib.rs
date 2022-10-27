#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use pink_extension as pink;

#[pink::contract(env = PinkEnvironment)]
#[pink(inner=ink_lang::contract)]
mod evm_chain {
    use super::pink;
    use alloc::vec;
    use alloc::vec::Vec;
    use ink_lang as ink;
    use pink::{http_get, PinkEnvironment};
    use traits::ensure;
    use traits::registry::{
        AssetInfo, AssetsRegisry, BalanceFetcher, ChainInspector, ChainType, Error as RegistryError,
    };
    use xcm::latest::{AssetId, MultiLocation};

    #[ink(storage)]
    // #[derive(SpreadAllocate)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct EvmChain {
        admin: AccountId,
        /// The chain name
        chain: Vec<u8>,
        /// Type of chain
        chain_type: ChainType,
        /// The registered assets list
        assets: Vec<AssetInfo>,
        /// Native asset of chain
        native: Option<AssetInfo>,
        /// Stable asset of chain
        stable: Option<AssetInfo>,
        /// RPC endpoint of chain
        endpoint: Vec<u8>,
    }

    /// Event emitted when native asset set.
    #[ink(event)]
    pub struct NativeSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    /// Event emitted when stable asset set.
    #[ink(event)]
    pub struct StableSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    /// Event emitted when RPC endpoint asset set.
    #[ink(event)]
    pub struct EndpointSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        endpoint: Vec<u8>,
    }

    /// Event emitted when asset registered.
    #[ink(event)]
    pub struct Registered {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    /// Event emitted when asset unregistered.
    #[ink(event)]
    pub struct Unregistered {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    pub type Result<T> = core::result::Result<T, RegistryError>;

    impl EvmChain {
        #[ink(constructor)]
        /// Create an Ethereum entity
        pub fn new(chain: Vec<u8>, endpoint: Vec<u8>) -> Self {
            EvmChain {
                admin: Self::env().caller(),
                chain,
                chain_type: ChainType::Evm,
                assets: vec![],
                native: None,
                stable: None,
                endpoint,
            }
        }

        /// Set native asset
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        pub fn set_native(&mut self, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;
            self.native = Some(asset.clone());
            Self::env().emit_event(NativeSet {
                chain: self.chain.clone(),
                asset: Some(asset),
            });
            Ok(())
        }

        /// Set native asset
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        pub fn set_stable(&mut self, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;
            self.stable = Some(asset.clone());
            Self::env().emit_event(StableSet {
                chain: self.chain.clone(),
                asset: Some(asset),
            });
            Ok(())
        }

        /// Set RPC endpoint
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        pub fn set_endpoint(&mut self, endpoint: Vec<u8>) -> Result<()> {
            self.esure_admin()?;
            self.endpoint = endpoint.clone();
            Self::env().emit_event(EndpointSet {
                chain: self.chain.clone(),
                endpoint,
            });
            Ok(())
        }

        /// Returns error if caller is not admin
        fn esure_admin(&self) -> Result<()> {
            let caller = self.env().caller();
            if self.admin != caller {
                return Err(RegistryError::BadOrigin);
            }
            Ok(())
        }
    }

    impl ChainInspector for EvmChain {
        /// Return admin of the chain
        #[ink(message)]
        fn owner(&self) -> AccountId {
            self.admin
        }

        /// Return name of the chain
        #[ink(message)]
        fn chain_name(&self) -> Vec<u8> {
            self.chain.clone()
        }

        /// Return set native asset of the chain
        #[ink(message)]
        fn chain_type(&self) -> ChainType {
            self.chain_type.clone()
        }

        /// Return set native asset of the chain
        #[ink(message)]
        fn native_asset(&self) -> Option<AssetInfo> {
            self.native.clone()
        }

        /// Return set stable asset of the chain
        #[ink(message)]
        fn stable_asset(&self) -> Option<AssetInfo> {
            self.stable.clone()
        }

        /// Return RPC endpoint of the chain
        #[ink(message)]
        fn endpoint(&self) -> Vec<u8> {
            self.endpoint.clone()
        }
    }

    impl BalanceFetcher for EvmChain {
        #[ink(message)]
        fn balance_of(&self, asset: AssetId, account: MultiLocation) -> Option<u128> {
            None
        }
    }

    impl AssetsRegisry for EvmChain {
        /// Register the asset
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        fn register(&mut self, asset: AssetInfo) -> core::result::Result<(), RegistryError> {
            self.esure_admin()?;

            ensure!(
                self.assets
                    .iter()
                    .position(|a| a.location == asset.location)
                    .is_none(),
                RegistryError::AssetAlreadyRegistered
            );
            self.assets.push(asset.clone());

            Self::env().emit_event(Registered {
                chain: self.chain.clone(),
                asset: Some(asset),
            });
            Ok(())
        }

        /// Unregister the asset
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        fn unregister(&mut self, asset: AssetInfo) -> core::result::Result<(), RegistryError> {
            self.esure_admin()?;

            let index = self
                .assets
                .iter()
                .position(|a| a.location == asset.location)
                .ok_or(RegistryError::AssetNotFound)?;
            self.assets.remove(index);

            Self::env().emit_event(Unregistered {
                chain: self.chain.clone(),
                asset: Some(asset),
            });
            Ok(())
        }

        /// Return all registerd assets
        #[ink(message)]
        fn registered_assets(&self) -> Vec<AssetInfo> {
            self.assets.clone()
        }

        #[ink(message)]
        fn lookup_by_name(&self, name: Vec<u8>) -> Option<AssetInfo> {
            self.assets
                .iter()
                .position(|a| a.name == name)
                .and_then(|idx| Some(self.assets[idx].clone()))
        }

        #[ink(message)]
        fn lookup_by_symbol(&self, symbol: Vec<u8>) -> Option<AssetInfo> {
            self.assets
                .iter()
                .position(|a| a.symbol == symbol)
                .and_then(|idx| Some(self.assets[idx].clone()))
        }

        #[ink(message)]
        fn lookup_by_location(&self, location: Vec<u8>) -> Option<AssetInfo> {
            self.assets
                .iter()
                .position(|a| a.location == location)
                .and_then(|idx| Some(self.assets[idx].clone()))
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use ink_lang as ink;

        type Event = <EvmChain as ink::reflect::ContractEventBase>::Type;

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
                    <Event as scale::Encode>::encode(&evt),
                    "Event data don't match"
                );
            }
        }

        #[ink::test]
        fn test_default_works() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            assert_eq!(ethereum.owner(), accounts.alice);
            assert_eq!(ethereum.chain_name(), b"Ethereum".to_vec());
            assert_eq!(ethereum.chain_type(), ChainType::Evm);
            assert_eq!(ethereum.native_asset(), None);
            assert_eq!(ethereum.stable_asset(), None);
            assert_eq!(ethereum.endpoint(), b"endpoint".to_vec());
        }

        #[ink::test]
        fn test_set_native_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            let weth = AssetInfo {
                name: b"Wrap Ether".to_vec(),
                symbol: b"WETH".to_vec(),
                decimals: 18,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(ethereum.set_native(weth.clone()), Ok(()));
            assert_events(vec![NativeSet {
                chain: b"Ethereum".to_vec(),
                asset: Some(weth.clone()),
            }
            .into()]);
            assert_eq!(ethereum.native_asset(), Some(weth));
        }

        #[ink::test]
        fn test_set_native_without_permisssion_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            let weth = AssetInfo {
                name: b"Wrap Ether".to_vec(),
                symbol: b"WETH".to_vec(),
                decimals: 18,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            set_caller(accounts.bob);
            assert_eq!(ethereum.set_native(weth), Err(RegistryError::BadOrigin));
        }

        #[ink::test]
        fn test_set_stable_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(ethereum.set_stable(usdc.clone()), Ok(()));
            assert_events(vec![StableSet {
                chain: b"Ethereum".to_vec(),
                asset: Some(usdc.clone()),
            }
            .into()]);
            assert_eq!(ethereum.stable_asset(), Some(usdc));
        }

        #[ink::test]
        fn test_set_stable_without_permisssion_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            set_caller(accounts.bob);
            assert_eq!(ethereum.set_stable(usdc), Err(RegistryError::BadOrigin));
        }

        #[ink::test]
        fn test_register_asset_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(ethereum.register(usdc.clone()), Ok(()));
            assert_events(vec![Registered {
                chain: b"Ethereum".to_vec(),
                asset: Some(usdc.clone()),
            }
            .into()]);
        }

        #[ink::test]
        fn test_set_endpoint_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            assert_eq!(ethereum.set_endpoint(b"new endpoint".to_vec()), Ok(()));

            assert_events(vec![EndpointSet {
                chain: b"Ethereum".to_vec(),
                endpoint: b"new endpoint".to_vec(),
            }
            .into()]);
            assert_eq!(ethereum.endpoint(), b"new endpoint".to_vec());
        }

        #[ink::test]
        fn test_set_endpoint_without_permisssion_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            set_caller(accounts.bob);
            assert_eq!(
                ethereum.set_endpoint(b"new endpoint".to_vec()),
                Err(RegistryError::BadOrigin)
            );
        }

        #[ink::test]
        fn test_duplicated_register_asset_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(ethereum.register(usdc.clone()), Ok(()));
            assert_eq!(
                ethereum.register(usdc),
                Err(RegistryError::AssetAlreadyRegistered)
            );
        }

        #[ink::test]
        fn test_register_asset_without_permission_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            set_caller(accounts.bob);

            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(ethereum.register(usdc), Err(RegistryError::BadOrigin));
        }

        #[ink::test]
        fn test_unregister_asset_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            assert_eq!(ethereum.register(usdc.clone()), Ok(()));
            assert_eq!(ethereum.unregister(usdc.clone()), Ok(()));

            assert_events(vec![
                Registered {
                    chain: b"Ethereum".to_vec(),
                    asset: Some(usdc.clone()),
                }
                .into(),
                Unregistered {
                    chain: b"Ethereum".to_vec(),
                    asset: Some(usdc),
                }
                .into(),
            ]);
        }

        #[ink::test]
        fn test_unregister_unregistered_asset_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            // First time unregister
            assert_eq!(
                ethereum.unregister(usdc.clone()),
                Err(RegistryError::AssetNotFound)
            );
            assert_eq!(ethereum.register(usdc.clone()), Ok(()));
            assert_eq!(ethereum.unregister(usdc.clone()), Ok(()));
            // Second time unregister
            assert_eq!(ethereum.unregister(usdc), Err(RegistryError::AssetNotFound));
        }

        #[ink::test]
        fn test_unregister_asset_without_permission_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            // Register by owner: alice
            assert_eq!(ethereum.register(usdc.clone()), Ok(()));
            set_caller(accounts.bob);
            // Bob trying to unregister
            assert_eq!(ethereum.unregister(usdc), Err(RegistryError::BadOrigin));
        }

        #[ink::test]
        fn test_unregister_asset_with_wrong_location_should_fail() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
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
            assert_eq!(ethereum.register(usdc.clone()), Ok(()));
            assert_eq!(
                ethereum.unregister(wrong_usdc),
                Err(RegistryError::AssetNotFound)
            );
        }

        #[ink::test]
        fn test_query_funtions_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec(), b"endpoint".to_vec());
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
            assert_eq!(ethereum.register(usdc.clone()), Ok(()));
            assert_eq!(ethereum.register(weth.clone()), Ok(()));
            assert_eq!(
                ethereum.registered_assets(),
                vec![usdc.clone(), weth.clone()]
            );
            assert_eq!(
                ethereum.lookup_by_name(weth.name.clone()),
                Some(weth.clone())
            );
            assert_eq!(ethereum.lookup_by_name(b"Wrong Name".to_vec()), None);
            assert_eq!(
                ethereum.lookup_by_symbol(weth.symbol.clone()),
                Some(weth.clone())
            );
            assert_eq!(ethereum.lookup_by_symbol(b"Wrong Symbol".to_vec()), None);
            assert_eq!(
                ethereum.lookup_by_location(weth.location.clone()),
                Some(weth.clone())
            );
            assert_eq!(
                ethereum.lookup_by_location(b"Wrong Location".to_vec()),
                None
            );
            assert_eq!(ethereum.unregister(usdc), Ok(()));
            assert_eq!(ethereum.registered_assets(), vec![weth.clone()]);
            assert_eq!(ethereum.unregister(weth), Ok(()));
            assert_eq!(ethereum.registered_assets(), vec![]);
        }
    }
}
