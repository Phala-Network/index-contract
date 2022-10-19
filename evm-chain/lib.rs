#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

mod eth_signer;

use pink_extension as pink;

#[pink::contract(env = PinkEnvironment)]
#[pink(inner=ink_lang::contract)]
mod evm_chain {
    use super::pink;
    use crate::eth_signer::EthSigner;
    use alloc::vec;
    use alloc::vec::Vec;
    use ink_lang as ink;
    use pink::PinkEnvironment;
    use traits::registry::{
        AssetInfo, AssetsRegisry, BalanceFetcher, ChainType, Error as RegistryError, Inspector,
        SignedTransaction,
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
        pub fn new() -> Self {
            EvmChain {
                admin: Self::env().caller(),
                chain: b"Ethereum".to_vec(),
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

    /// Same as Signer trait
    #[ink::trait_definition]
    pub trait EthTx {
        /// Sign a transaction
        #[ink(message)]
        fn sign_transaction(&self, signer: EthSigner, unsigned_tx: Vec<u8>) -> SignedTransaction;
    }

    impl EthTx for EvmChain {
        #[ink(message)]
        fn sign_transaction(&self, signer: EthSigner, unsigned_tx: Vec<u8>) -> SignedTransaction {
            // TODO: sign with signer
            SignedTransaction::EthSignedTransaction
        }
    }

    impl Inspector for EvmChain {
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
}
