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
    use pink::PinkEnvironment;
    use traits::registry::{
        AssetInfo, AssetsRegisry, BalanceFetcher, ChainInspector, ChainType, Error as RegistryError,
    };

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
    }

    /// Event emitted when a token transfer occurs.
    #[ink(event)]
    pub struct NativeSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    /// Event emitted when a token transfer occurs.
    #[ink(event)]
    pub struct StableSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    pub type Result<T> = core::result::Result<T, RegistryError>;

    impl EvmChain {
        #[ink(constructor)]
        /// Create an Ethereum entity
        pub fn new(chain: Vec<u8>) -> Self {
            EvmChain {
                admin: Self::env().caller(),
                chain: chain,
                chain_type: ChainType::Evm,
                assets: vec![],
                native: None,
                stable: None,
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
    }

    // impl BalanceFetcher for EvmChain {

    // }

    // impl AssetsRegisry<(), Error> for EvmChain {

    // }

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
            let ethereum = EvmChain::new(b"Ethereum".to_vec());
            assert_eq!(ethereum.owner(), accounts.alice);
            assert_eq!(ethereum.chain_name(), b"Ethereum".to_vec());
            assert_eq!(ethereum.chain_type(), ChainType::Evm);
            assert_eq!(ethereum.native_asset(), None);
            assert_eq!(ethereum.stable_asset(), None);
        }

        #[ink::test]
        fn test_set_native_should_work() {
            let accounts = default_accounts();
            set_caller(accounts.alice);
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec());
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
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec());
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
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec());
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
            let mut ethereum = EvmChain::new(b"Ethereum".to_vec());
            let usdc = AssetInfo {
                name: b"USD Coin".to_vec(),
                symbol: b"USDC".to_vec(),
                decimals: 6,
                location: b"Somewhere on Ethereum".to_vec(),
            };
            set_caller(accounts.bob);
            assert_eq!(ethereum.set_stable(usdc), Err(RegistryError::BadOrigin));
        }
    }
}
