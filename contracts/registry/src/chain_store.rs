extern crate alloc;

use crate::types::{AssetInfo, ChainInfo, Error as RegistryError};
use alloc::vec;
use alloc::{string::String, vec::Vec};
use index::ensure;
use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct ChainStore {
    pub info: ChainInfo,
    pub assets: Vec<AssetInfo>,
}

impl ChainStore {
    /// Create an ChainStore entity
    pub fn new(info: ChainInfo) -> Self {
        let mut assets: Vec<AssetInfo> = vec![];
        if let Some(ref stable) = info.stable {
            assets.push(stable.clone())
        }
        if let Some(ref native) = info.native {
            assets.push(native.clone())
        }
        ChainStore { info, assets }
    }

    pub fn get_info(&self) -> ChainInfo {
        self.info.clone()
    }

    pub fn set_native(&mut self, native: AssetInfo) {
        self.info.native = Some(native);
    }

    pub fn set_stable(&mut self, stable: AssetInfo) {
        self.info.stable = Some(stable);
    }

    pub fn set_endpoint(&mut self, endpoint: String) {
        self.info.endpoint = endpoint;
    }

    /// Register the asset
    pub fn register(&mut self, asset: AssetInfo) -> core::result::Result<(), RegistryError> {
        ensure!(
            !self.assets.iter().any(|a| a.location == asset.location),
            RegistryError::AssetAlreadyRegistered
        );
        self.assets.push(asset);
        Ok(())
    }

    /// Unregister the asset
    pub fn unregister(&mut self, asset: AssetInfo) -> core::result::Result<(), RegistryError> {
        let index = self
            .assets
            .iter()
            .position(|a| a.location == asset.location)
            .ok_or(RegistryError::AssetNotFound)?;
        self.assets.remove(index);
        Ok(())
    }

    /// Return all registerd assets
    pub fn registered_assets(&self) -> Vec<AssetInfo> {
        self.assets.clone()
    }

    pub fn lookup_by_name(&self, name: String) -> Option<AssetInfo> {
        self.assets
            .iter()
            .position(|a| a.name == name)
            .map(|idx| self.assets[idx].clone())
    }

    pub fn lookup_by_symbol(&self, symbol: String) -> Option<AssetInfo> {
        self.assets
            .iter()
            .position(|a| a.symbol == symbol)
            .map(|idx| self.assets[idx].clone())
    }

    pub fn lookup_by_location(&self, location: Vec<u8>) -> Option<AssetInfo> {
        self.assets
            .iter()
            .position(|a| a.location == location)
            .map(|idx| self.assets[idx].clone())
    }
}
