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
pub struct AssetPair {
    pub asset0: AssetInfo,
    pub asset1: AssetInfo,
}

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct Bridge {
    pub id: Vec<u8>,
    pub chain0: ChainInfo,
    pub chain1: ChainInfo,
    pub assets: Vec<AssetPair>,
}

impl Bridge {
    pub fn new(id: Vec<u8>, chain0: ChainInfo, chain1: ChainInfo) -> Self {
        Bridge { 
            id,
            chain0,
            chain1,
            assets: vec![],
         }
    }

    pub fn register(_pair: AssetPair) -> core::result::Result<(), RegistryError> {
        Err(RegistryError::Unimplemented)
    }

    pub fn unregister(_pair: AssetPair) -> core::result::Result<(), RegistryError> {
        Err(RegistryError::Unimplemented)
    }

    /// Return asset pair that the giving asset paired to
    pub fn get_pair(_asset: AssetInfo) -> Option<AssetPair> {
        None
    }

    /// Return bridge capacity of the given asset
    pub fn get_capacity(_asset: AssetInfo) -> core::result::Result<u128, RegistryError> {
        Err(RegistryError::Unimplemented)
    }
}