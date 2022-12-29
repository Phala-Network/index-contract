#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use ink_lang as ink;

pub use registry::{Graph, Registry, RegistryRef};
pub mod error;

#[allow(clippy::large_enum_variant)]
#[ink::contract(env = pink_extension::PinkEnvironment)]
mod registry {
    use crate::error::Error;
    use alloc::{string::String, vec::Vec};
    use index::ensure;
    use ink_storage::traits::PackedAllocate;
    use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};
    use ink_storage::Mapping;

    #[derive(Debug, Clone, Default, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Chain {
        pub id: u32,
        pub name: String,
        pub chain_type: u32,
    }

    #[derive(Debug, Clone, Default, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Asset {
        pub id: u32,
        pub symbol: String,
        pub name: String,
        pub location: String,
        pub decimals: u32,
        pub chain_id: u32,
    }

    #[derive(Debug, Clone, Default, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Dex {
        pub id: u32,
        pub name: String,
        pub chain_id: u32,
    }

    #[derive(Debug, Clone, Default, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct DexIndexer {
        pub id: u32,
        pub url: String,
        pub dex_id: u32,
    }

    #[derive(Debug, Clone, Default, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct DexPair {
        pub id: u32,
        pub asset0_id: u32,
        pub asset1_id: u32,
        pub dex_id: u32,
        pub pair_id: String,
    }

    #[derive(Debug, Clone, Default, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Bridge {
        pub id: u32,
        pub name: String,
        pub location: String,
    }

    #[derive(Debug, Clone, Default, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct BridgePair {
        pub id: u32,
        pub asset0_id: u32,
        pub asset1_id: u32,
        pub bridge_id: u32,
    }

    #[derive(Debug, Clone, Default, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct Graph {
        pub chains: Vec<Chain>,
        pub assets: Vec<Asset>,
        pub dexs: Vec<Dex>,
        pub dex_pairs: Vec<DexPair>,
        pub dex_indexers: Vec<DexIndexer>,
        pub bridges: Vec<Bridge>,
        pub bridge_pairs: Vec<BridgePair>,
    }

    /// Event emitted when graph is set.
    #[ink(event)]
    pub struct GraphSet;

    #[ink(storage)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Registry {
        pub admin: AccountId,
        graph: Graph,
    }

    impl Default for Registry {
        fn default() -> Self {
            Self::new()
        }
    }

    pub type Result<T> = core::result::Result<T, Error>;

    impl Registry {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                admin: Self::env().caller(),
                graph: Graph::default(),
            }
        }

        /// Returns error if caller is not admin
        fn ensure_admin(&self) -> Result<()> {
            let caller = self.env().caller();
            if self.admin != caller {
                return Err(Error::BadOrigin);
            }
            Ok(())
        }

        /// Sets the graph, callable only to a specifically crafted management tool,
        /// should not be called by anyone else
        #[ink(message)]
        pub fn set_graph(&mut self, graph: Graph) -> Result<()> {
            self.ensure_admin()?;
            self.graph = graph;
            Self::env().emit_event(GraphSet {});
            Ok(())
        }

        /// Returs the interior graph, callable to all
        #[ink(message)]
        pub fn get_graph(&self) -> Graph {
            return self.graph.clone();
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use dotenv::dotenv;
        use ink_lang as ink;
        use phala_pallet_common::WrapSlice;
        use pink_extension::PinkEnvironment;
        use scale::Encode;

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
    }
}
