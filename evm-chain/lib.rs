#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use pink_extension as pink;

#[pink::contract(env = PinkEnvironment)]
#[pink(inner=ink_lang::contract)]
mod evm_chain {
    use super::pink;
    use alloc::vec;
    use alloc::{string::String, vec::Vec};
    use ink_lang as ink;
    use phala_pallet_common::WrapSlice;
    use pink::{http_get, PinkEnvironment};
    use pink_web3::api::{Eth, Namespace};
    use pink_web3::contract::{Contract, Options};
    use pink_web3::transports::{resolve_ready, PinkHttp};
    use pink_web3::types::{Address, Res, H256};
    use scale::{Decode, Encode};
    use traits::ensure;
    use traits::registry::{
        AssetInfo, AssetsRegisry, BalanceFetcher, ChainInspector, ChainType, Error as RegistryError,
    };
    use xcm::latest::{prelude::*, Fungibility::Fungible, MultiAsset, MultiLocation};

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

        /// An asset id represented by MultiLocation like:
        /// (1, X4(Parachain(phala_id), GeneralKey(“phat"), GeneralKey(cluster_id), GeneralKey(erc20_address)))
        fn extract_token(&self, asset: &AssetId) -> Option<Address> {
            match asset {
                Concrete(location) => {
                    match (location.parents, &location.interior) {
                        (
                            1,
                            Junctions::X4(
                                Parachain(_id),
                                GeneralKey(_phat_key),
                                GeneralKey(_cluster_id),
                                GeneralKey(erc20_address),
                            ),
                        ) => {
                            // TODO.wf verify arguments
                            if erc20_address.len() != 20 {
                                return None;
                            };
                            let address: Address = Address::from_slice(&erc20_address);
                            Some(address)
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }

        /// An account location represented by MultiLocation like:
        /// (1, X4(Parachain(phala_id), GeneralKey(“phat"), GeneralKey(cluster_id), GeneralKey(account_address)))
        fn extract_account(&self, location: &MultiLocation) -> Option<Address> {
            match (location.parents, &location.interior) {
                (
                    1,
                    Junctions::X4(
                        Parachain(_id),
                        GeneralKey(_phat_key),
                        GeneralKey(_cluster_id),
                        GeneralKey(account_address),
                    ),
                ) => {
                    // TODO.wf verify arguments
                    if account_address.len() != 20 {
                        return None;
                    };
                    let address: Address = Address::from_slice(&account_address);
                    Some(address)
                }
                _ => None,
            }
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
        fn balance_of(&self, asset: AssetId, account: MultiLocation) -> Result<u128> {
            // let eth = Eth::new(PinkHttp::new(String::from_utf8_lossy(&self.endpoint)));
            let eth = Eth::new(PinkHttp::new(
                "https://mainnet.infura.io/v3/6d61e7957c1c489ea8141e947447405b",
            ));

            // let abi_str = r#"[
            //     {
            //         "type": "function",
            //         "name": "balanceOf",
            //         "constant":true,
            //         "stateMutability": "view",
            //         "payable":false, "inputs": [
            //           { "type": "address", "name": "owner"}
            //         ],
            //         "outputs": [
            //           { "type": "uint256"}
            //         ]
            //       }
            // ]"#;
            // let abi =
            //     pink_json::from_str(abi_str).map_err(|_| RegistryError::InvalidContractAbi)?;
            // let abi = include_bytes!("./res/erc20-abi.json");
            let token_address: Address = self
                .extract_token(&asset)
                .ok_or(RegistryError::ExtractLocationFailed)?;
            let account: Address = self
                .extract_account(&account)
                .ok_or(RegistryError::ExtractLocationFailed)?;
            let erc20 = Contract::from_json(
                eth,
                // PHA address
                hex_literal::hex!["6c5bA91642F10282b576d91922Ae6448C9d52f4E"].into(),
                include_bytes!("./res/erc20-abi.json"),
            )
            .map_err(|_| RegistryError::ConstructContractFailed)?;
            // TODO.wf handle potential failure smoothly instead of unwrap directly
            let result: String =
                resolve_ready(erc20.query("balanceOf", account, None, Options::default(), None))
                    .unwrap();
            Ok(result.parse::<u128>().expect("U128 convert failed"))
        }
    }

    impl AssetsRegisry for EvmChain {
        /// Register the asset
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        fn register(&mut self, asset: AssetInfo) -> Result<()> {
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
        fn unregister(&mut self, asset: AssetInfo) -> Result<()> {
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
        use dotenv::dotenv;
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

        #[ink::test]
        fn test_query_balance_should_work() {
            dotenv().ok();
            use std::env;

            pink_extension_runtime::mock_ext::mock_all_ext();

            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(
                b"Ethereum".to_vec(),
                b"https://mainnet.infura.io/v3/6d61e7957c1c489ea8141e947447405b".to_vec(),
            );
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

            assert_eq!(ethereum.register(pha.clone()), Ok(()));
            assert_eq!(
                ethereum.balance_of(AssetId::Concrete(pha_location), account_location),
                Ok(35_000_000_000_000_000u128)
            );
        }
    }
}
