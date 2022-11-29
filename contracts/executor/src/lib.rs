#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use ink_lang as ink;

mod task;

#[ink::contract(env = pink_extension::PinkEnvironment)]
mod index_executor {
    use alloc::{vec, vec::Vec};
    use scale::{Decode, Encode};
    // use index::{ensure, prelude::*};
    use index_registry::{types::Graph, RegistryRef};
    use ink_env::call::FromAccountId;

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        ExecuteFailed,
        Unimplemented,
    }

    type Result<T> = core::result::Result<T, Error>;

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
            for _index in 0..10 {
                // TODO: generate private key
                worker_accounts.push([0; 32])
            }

            Self {
                admin: Self::env().caller(),
                registry: None,
                worker_accounts,
                executor_account: [0; 32],
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
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use dotenv::dotenv;
        use index_registry::{
            types::{ChainInfo, ChainType},
            Registry,
        };
        use pink_extension::PinkEnvironment;

        fn default_accounts() -> ink_env::test::DefaultAccounts<PinkEnvironment> {
            ink_env::test::default_accounts::<PinkEnvironment>()
        }

        fn set_caller(sender: AccountId) {
            ink_env::test::set_caller::<PinkEnvironment>(sender);
        }
    }
}
