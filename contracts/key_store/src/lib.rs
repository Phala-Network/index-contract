#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

pub use crate::key_store::{KeyStore, KeyStoreRef};

#[ink::contract(env = pink_extension::PinkEnvironment)]
mod key_store {
    use alloc::{vec, vec::Vec};
    use scale::{Decode, Encode};

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        BadOrigin,
        MissingExecutor,
    }

    type Result<T> = core::result::Result<T, Error>;

    #[ink(storage)]
    pub struct KeyStore {
        pub admin: AccountId,
        pub prv_keys: Vec<[u8; 32]>,
        pub executor: Option<[u8; 32]>,
    }

    impl Default for KeyStore {
        fn default() -> Self {
            Self::default()
        }
    }

    impl KeyStore {
        #[allow(clippy::should_implement_trait)]
        #[ink(constructor)]
        pub fn default() -> Self {
            let mut prv_keys: Vec<[u8; 32]> = vec![];

            for index in 0..10 {
                let private_key = pink_web3::keys::pink::KeyPair::derive_keypair(
                    &[b"worker".to_vec(), [index].to_vec()].concat(),
                )
                .private_key();
                prv_keys.push(private_key);
            }

            Self {
                admin: Self::env().caller(),
                prv_keys,
                executor: None,
            }
        }

        #[ink(message)]
        pub fn transfer_ownership(&mut self, new_admin: AccountId) -> Result<()> {
            self.ensure_owner()?;
            self.admin = new_admin;
            Ok(())
        }

        #[ink(message)]
        pub fn set_executor(&mut self, executor: [u8; 32]) -> Result<()> {
            self.ensure_owner()?;
            self.executor = Some(executor);
            Ok(())
        }

        #[ink(message)]
        pub fn remove_executor(&mut self) -> Result<()> {
            self.ensure_owner()?;
            self.executor = None;
            Ok(())
        }

        /// Only the whitelisted executor are allowed to call this function
        #[ink(message)]
        pub fn get_worker_keys(&self) -> Result<Vec<[u8; 32]>> {
            self.ensure_executor()?;
            Ok(self.prv_keys.clone())
        }

        #[ink(message)]
        pub fn get_executor(&self) -> Result<Option<[u8; 32]>> {
            Ok(self.executor)
        }

        /// Returns BadOrigin error if the caller is not the owner
        fn ensure_owner(&self) -> Result<()> {
            if self.env().caller() == self.admin {
                Ok(())
            } else {
                Err(Error::BadOrigin)
            }
        }

        /// Returns BadOrigin error if the caller is not the owner
        fn ensure_executor(&self) -> Result<()> {
            let executor = self.executor.ok_or(Error::MissingExecutor)?;
            if self.env().caller() == executor.into() {
                Ok(())
            } else {
                Err(Error::BadOrigin)
            }
        }
    }
}
