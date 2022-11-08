#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use crate::ensure;
use crate::traits::{
    common::Error as RegistryError,
    registry::{AssetInfo, AssetsRegisry, BalanceFetcher, ChainInfo, ChainInspector, ChainMutate},
};
use alloc::vec;
use alloc::vec::Vec;
use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};
use xcm::latest::{prelude::*, MultiLocation};

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct SubChain {
    pub info: ChainInfo,
    pub assets: Vec<AssetInfo>,
}

impl SubChain {
    /// Create an SubChain entity
    pub fn new(info: ChainInfo) -> Self {
        let mut assets: Vec<AssetInfo> = vec![];
        if let Some(ref stable) = info.stable {
            assets.push(stable.clone())
        }
        if let Some(ref native) = info.native {
            assets.push(native.clone())
        }
        SubChain { info, assets }
    }
}

impl ChainInspector for SubChain {
    fn get_info(&self) -> ChainInfo {
        self.info.clone()
    }
}

impl ChainMutate for SubChain {
    fn set_native(&mut self, native: AssetInfo) {
        self.info.native = Some(native);
    }

    fn set_stable(&mut self, stable: AssetInfo) {
        self.info.stable = Some(stable);
    }

    fn set_endpoint(&mut self, endpoint: Vec<u8>) {
        self.info.endpoint = endpoint;
    }
}

impl BalanceFetcher for SubChain {
    fn balance_of(
        &self,
        _asset: AssetId,
        _account: MultiLocation,
    ) -> core::result::Result<u128, RegistryError> {
        Err(RegistryError::Unimplemented)
    }
}

impl AssetsRegisry for SubChain {
    /// Register the asset
    fn register(&mut self, asset: AssetInfo) -> core::result::Result<(), RegistryError> {
        ensure!(
            !self.assets.iter().any(|a| a.location == asset.location),
            RegistryError::AssetAlreadyRegistered
        );
        self.assets.push(asset);
        Ok(())
    }

    /// Unregister the asset
    fn unregister(&mut self, asset: AssetInfo) -> core::result::Result<(), RegistryError> {
        let index = self
            .assets
            .iter()
            .position(|a| a.location == asset.location)
            .ok_or(RegistryError::AssetNotFound)?;
        self.assets.remove(index);
        Ok(())
    }

    /// Return all registerd assets
    fn registered_assets(&self) -> Vec<AssetInfo> {
        self.assets.clone()
    }

    fn lookup_by_name(&self, name: Vec<u8>) -> Option<AssetInfo> {
        self.assets
            .iter()
            .position(|a| a.name == name)
            .map(|idx| self.assets[idx].clone())
    }

    fn lookup_by_symbol(&self, symbol: Vec<u8>) -> Option<AssetInfo> {
        self.assets
            .iter()
            .position(|a| a.symbol == symbol)
            .map(|idx| self.assets[idx].clone())
    }

    fn lookup_by_location(&self, location: Vec<u8>) -> Option<AssetInfo> {
        self.assets
            .iter()
            .position(|a| a.location == location)
            .map(|idx| self.assets[idx].clone())
    }
}
