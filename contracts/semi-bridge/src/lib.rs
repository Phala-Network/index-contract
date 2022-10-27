#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use ink_lang as ink;

#[ink::contract(env = pink_extension::PinkEnvironment)]
mod semi_bridge {
    use alloc::{string::String, string::ToString, vec::Vec};
    use index::v0::prelude::*;
    use index::v0::utils::ToArray;
    use ink_storage::traits::{PackedLayout, SpreadLayout};
    use pink_web3::ethabi::{Bytes, Uint};
    use pink_web3::futures::executor;
    use pink_web3::keys::pink::KeyPair;
    use pink_web3::signing::Key;
    use primitive_types::{H160, H256, U256};
    use scale::{Decode, Encode};

    #[ink(storage)]
    pub struct SemiBridge {
        owner: AccountId,
        key: [u8; 32],
        config: Option<Config>,
    }

    #[derive(Encode, Decode, Debug, PackedLayout, SpreadLayout)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
    )]
    struct Config {
        rpc: String,
        bridge_address: [u8; 20],
    }

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        BadOrigin,
        NotConfigurated,
        KeyRetired,
        KeyNotRetiredYet,
        UpstreamFailed,
        BadAbi,
        FailedToGetStorage,
        FailedToDecodeStorage,
        FailedToEstimateGas,
        FailedToCreateExecutor,
    }

    type Result<T> = core::result::Result<T, Error>;

    impl SemiBridge {
        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                owner: Self::env().caller(),
                config: None,
                key: Self::key_pair().private_key(),
            }
        }

        /// Configures the bridge
        #[ink(message)]
        pub fn config(&mut self, rpc: String, bridge_address: H160) -> Result<()> {
            self.ensure_owner()?;
            self.config = Some(Config {
                rpc,
                bridge_address: bridge_address.into(),
            });
            Ok(())
        }

        /// Import a private key to override the interior account
        #[ink(message)]
        pub fn set_account(&mut self, private_key: Vec<u8>) -> H160 {
            self.key = private_key.to_array();
            self.wallet()
        }

        /// Returns the wallet address
        #[ink(message)]
        pub fn wallet(&self) -> H160 {
            let keypair: KeyPair = self.key.into();
            keypair.address()
        }

        /// Returns BadOrigin error if the caller is not the owner
        fn ensure_owner(&self) -> Result<()> {
            if self.env().caller() == self.owner {
                Ok(())
            } else {
                Err(Error::BadOrigin)
            }
        }

        /// Derives the key pair on the fly
        fn key_pair() -> pink_web3::keys::pink::KeyPair {
            pink_web3::keys::pink::KeyPair::derive_keypair(b"rollup-bridge")
        }

        /// Transfers tokens to the `address` derived from the contract's private key
        ///
        /// # Arguments
        ///
        /// * `src_chain`: an integer that represents the chain from which the asset is transferred
        /// * `dest_chain`: the recipient of the tokens
        /// * `token_rid`: token resource id
        /// * `amount`: amount of token to be transferred
        #[ink(message)]
        pub fn transfer(
            &self,
            src_chain: u8,
            dest_chain: u8,
            token_rid: H256,
            amount: U256,
        ) -> Result<()> {
            let config = self
                .config
                .as_ref()
                .map(Ok)
                .unwrap_or(Err(Error::NotConfigurated))?;
            let executor = Evm2PhalaExecutor::new(
                Address::EthAddr(config.bridge_address.into()),
                include_bytes!("../res/evm_contract.abi.json"),
                &config.rpc,
                src_chain,
                dest_chain,
            )
            .or(Err(Error::FailedToCreateExecutor))?;
            _ = executor.transfer(self.key, token_rid, amount);
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink_lang as ink;

        // todo: add unit tests
        #[ink::test]
        fn it_works() {
            pink_extension_runtime::mock_ext::mock_all_ext();
            //pink_extension::chain_extension::mock::mock_derive_sr25519_key(|_| {
            //    hex!["4c5d4f158b3d691328a1237d550748e019fe499ebf3df7467db6fa02a0818821"].to_vec()
            //});
        }
    }
}
