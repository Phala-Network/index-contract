#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[ink::contract(env = pink_extension::PinkEnvironment)]
mod system_remark {
    use pink_subrpc as subrpc;

    use alloc::{string::String, vec::Vec};
    use scale::{Decode, Encode};
    use subrpc::{create_transaction, send_transaction, ExtraParam};

    #[ink(storage)]
    pub struct Remarker {
        pub admin: AccountId,
    }

    #[derive(Encode, Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        // To fix metadata empty variants
        Unimplemented,
    }

    type Result<T> = core::result::Result<T, Error>;

    impl Remarker {
        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                admin: Self::env().caller(),
            }
        }

        /// Sends a remark to the chain
        ///
        /// Just to make sure `cargo contract build` can work
        #[ink(message)]
        pub fn remark(&self, remark: String) -> Result<Vec<u8>> {
            use hex_literal::hex;
            let rpc_node = "https://khala.api.onfinality.io:443/public-ws";
            let signer: [u8; 32] =
                hex!("9eb2ee60393aeeec31709e256d448c9e40fa64233abf12318f63726e9c417b69");
            let signed_tx = create_transaction(
                &signer,
                "khala",
                rpc_node,
                0u8,
                0u8,
                remark,
                ExtraParam::default(),
            )
            .unwrap();
            let tx_id = send_transaction(rpc_node, &signed_tx).unwrap();
            Ok(tx_id)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use dotenv::dotenv;
        use hex_literal::hex;
        // use pink_web3::ethabi::Uint;
        // use std::time::SystemTime;

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
            let remarker = Remarker::default();

            let tx_id = remarker
                .remark("Greetings from Phat Contract!".to_string())
                .unwrap();
            dbg!(hex::encode(&tx_id));
        }
    }
}
