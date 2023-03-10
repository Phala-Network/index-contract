#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[ink::contract(env = pink_extension::PinkEnvironment)]
mod semi_bridge {
    use alloc::{string::String, vec, vec::Vec};
    use index::prelude::*;
    use index::utils::ToArray;
    use pink_subrpc::ExtraParam;
    use pink_web3::keys::pink::KeyPair;
    use pink_web3::signing::Key;
    use pink_web3::types::{H160, H256, U256};
    use scale::{Decode, Encode};

    #[ink(storage)]
    pub struct SemiBridge {
        owner: AccountId,
        key: [u8; 32],
        config: Option<Config>,
    }

    #[derive(Encode, Decode, Debug)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
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
        /// * `token_contract`: token contract address
        /// * `token_rid`: token resource id
        /// * `amount`: amount of token to be transferred
        /// * `recipient`: the account that receives the tokens on Phala chain
        #[ink(message)]
        pub fn transfer(
            &self,
            token_contract: H160,
            token_rid: H256,
            amount: U256,
            recipient: Vec<u8>,
        ) -> Result<()> {
            let config = self.config.as_ref().ok_or(Error::NotConfigurated)?;
            let executor = ChainBridgeEthereum2Phala::new(
                &config.rpc,
                CHAINBRIDGE_ID_KHALA,
                config.bridge_address.into(),
                vec![(token_contract, token_rid.into())],
            );
            _ = executor.transfer(
                self.key,
                token_contract.as_bytes().to_vec(),
                recipient,
                amount.try_into().expect("Amount converted failed"),
                ExtraParam::default(),
            );
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use dotenv::dotenv;
        use hex_literal::hex;
        use pink_web3::ethabi::Uint;

        #[ink::test]
        #[ignore]
        /// We are not going to mock in this sample,
        /// real life configuration is important for the understanding of use cases.
        /// this test also runs in CI, so it must not panic
        fn it_works() {
            dotenv().ok();

            pink_extension_runtime::mock_ext::mock_all_ext();
            pink_extension::chain_extension::mock::mock_derive_sr25519_key(|_| {
                hex!["4c5d4f158b3d691328a1237d550748e019fe499ebf3df7467db6fa02a0818821"].to_vec()
            });

            // Deploy Transactor(phat contract)
            let mut bridge = SemiBridge::default();

            let rpc =
                "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2".to_string();
            let bridge_contract_addr: H160 =
                hex!("056c0e37d026f9639313c281250ca932c9dbe921").into();

            bridge.config(rpc, bridge_contract_addr).unwrap();
            let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");

            if secret_key.is_none() {
                return Ok(());
            }

            let secret_key = secret_key.unwrap().1;
            let secret_bytes = hex::decode(secret_key).unwrap();
            bridge.set_account(secret_bytes);
            // PHA ChainBridge resource id on Khala
            let token_rid: H256 =
                hex!("00e6dfb61a2fb903df487c401663825643bb825d41695e63df8af6162ab145a6").into();
            // PHA contract address on Ethereum
            let token_contract: H160 = hex!("6c5bA91642F10282b576d91922Ae6448C9d52f4E").into();
            // 1 PHA
            let amount = Uint::from(1_000_000_000_000_000_000_u128);

            let recipient: Vec<u8> =
                hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48").into();

            // an example result:
            // https://goerli.etherscan.io/tx/0xc064af26458ca91b86af128ba86d9cdcee51397cebebc714df8fc182b298ab11
            _ = bridge.transfer(token_contract, token_rid, amount, recipient);
        }
    }
}
