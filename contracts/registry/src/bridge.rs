extern crate alloc;

use crate::types::{AssetInfo, ChainInfo, Error as RegistryError};
use alloc::vec;
use alloc::{string::String, vec::Vec};
use index::ensure;
use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct AssetPair {
    pub asset0: AssetInfo,
    pub asset1: AssetInfo,
}

impl AssetPair {
    pub fn id(&self) -> [u8; 32] {
        // FIXME: id generation shouldn't be determined by sequence of token pair
        sp_core_hashing::blake2_256(
            &[self.asset0.location.clone(), self.asset1.location.clone()].concat(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct Bridge {
    pub name: String,
    pub chain0: ChainInfo,
    pub chain1: ChainInfo,
    pub assets: Vec<AssetPair>,
}

impl Bridge {
    pub fn new(name: String, chain0: ChainInfo, chain1: ChainInfo) -> Self {
        Bridge {
            name,
            chain0,
            chain1,
            assets: vec![],
        }
    }

    pub fn register(&mut self, pair: AssetPair) -> core::result::Result<(), RegistryError> {
        ensure!(
            !self.assets.iter().any(|p| p.id() == pair.id()),
            RegistryError::AssetAlreadyRegistered
        );
        self.assets.push(pair);
        Ok(())
    }

    pub fn unregister(&mut self, pair: AssetPair) -> core::result::Result<(), RegistryError> {
        let index = self
            .assets
            .iter()
            .position(|p| p.id() == pair.id())
            .ok_or(RegistryError::AssetNotFound)?;
        self.assets.remove(index);
        Ok(())
    }

    /// Return asset pair that the giving asset paired to
    pub fn get_pair(_asset: AssetInfo) -> Option<AssetPair> {
        None
    }
}
