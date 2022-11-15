#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use crate::traits::{
    common::Error as RegistryError,
    registry::{AssetInfo, ChainInfo},
};
use alloc::{string::String, vec::Vec};
use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct DexPair {
    /// Address or other representation of a trading pair, should be the only indentification of the pair
    pub id: Vec<u8>,
    // Base trading token
    pub asset0: AssetInfo,
    // Quote trading token
    pub asset1: AssetInfo,
    // Potential swap fee
    pub swap_fee: Option<u128>,
    // Potential Dev fee
    pub dev_fee: Option<u128>,
}

impl DexPair {
    /// Flip the whole trading pair
    pub fn flip(&self) -> DexPair {
        DexPair {
            asset0: self.asset1.clone(),
            asset1: self.asset0.clone(),
            ..self.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct Dex {
    /// Factory contract address or a location(e.g. Pallet location on Polkadot ecosystem),
    /// should be used as the only indentification of a Dex
    pub id: Vec<u8>,
    /// Name of the DEX
    pub name: Vec<u8>,
    /// The chain that DEX deployed on
    pub chain: ChainInfo,
    /// Registered trading pairs
    pub pairs: Vec<DexPair>,
}

impl Dex {
    pub fn new(name: Vec<u8>, id: Vec<u8>, chain: ChainInfo) -> Self {
        Dex { 
            id,
            name,
            chain,
            pairs: vec![],
         }
    }

    pub fn register(_pair: DexPair) -> core::result::Result<(), RegistryError> {
        Err(RegistryError::Unimplemented)
    }

    pub fn unregister(_pair: DexPair) -> core::result::Result<(), RegistryError> {
        Err(RegistryError::Unimplemented)
    }

    /// Return asset pair that the giving asset paired to
    pub fn get_pair(_pair_id: Vec<u8>) -> Option<DexPair> {
        None
    }

    /// Return dex capacity of the given trading pair
    pub fn get_capacities(_pair_id: Vec<u8>) -> core::result::Result<(), RegistryError> {
        Err(RegistryError::Unimplemented)
    }
}