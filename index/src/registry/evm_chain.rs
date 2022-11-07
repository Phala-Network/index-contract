#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use crate::ensure;
use crate::traits::{
    common::Error as RegistryError,
    registry::{AssetInfo, AssetsRegisry, BalanceFetcher, ChainInfo, ChainInspector, ChainMutate},
};
use alloc::vec;
use alloc::{string::String, vec::Vec};
use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};
use pink_web3::api::{Eth, Namespace};
use pink_web3::contract::{Contract, Options};
use pink_web3::transports::{resolve_ready, PinkHttp};
use pink_web3::types::Address;
use xcm::latest::{prelude::*, MultiLocation};

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct EvmChain {
    pub info: ChainInfo,
    pub assets: Vec<AssetInfo>,
}

impl EvmChain {
    /// Create an EvmChain entity
    pub fn new(info: ChainInfo) -> Self {
        let mut assets: Vec<AssetInfo> = vec![];
        if let Some(ref stable) = info.stable {
            assets.push(stable.clone())
        }
        if let Some(ref native) = info.native {
            assets.push(native.clone())
        }
        EvmChain { info, assets }
    }

    /// An asset id represented by MultiLocation like:
    /// (1, X4(Parachain(phala_id), GeneralKey(“phat"), GeneralKey(cluster_id), GeneralKey(erc20_address)))
    fn extract_token(&self, asset: &AssetId) -> Option<Address> {
        match asset {
            Concrete(location) => {
                match (location.parents, &location.interior) {
                    (
                        1,
                        Junctions::X4(
                            Parachain(_id),
                            GeneralKey(_phat_key),
                            GeneralKey(_cluster_id),
                            GeneralKey(erc20_address),
                        ),
                    ) => {
                        // TODO.wf verify arguments
                        if erc20_address.len() != 20 {
                            return None;
                        };
                        let address: Address = Address::from_slice(erc20_address);
                        Some(address)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// An account location represented by MultiLocation like:
    /// (1, X4(Parachain(phala_id), GeneralKey(“phat"), GeneralKey(cluster_id), GeneralKey(account_address)))
    fn extract_account(&self, location: &MultiLocation) -> Option<Address> {
        match (location.parents, &location.interior) {
            (
                1,
                Junctions::X4(
                    Parachain(_id),
                    GeneralKey(_phat_key),
                    GeneralKey(_cluster_id),
                    GeneralKey(account_address),
                ),
            ) => {
                // TODO.wf verify arguments
                if account_address.len() != 20 {
                    return None;
                };
                let address: Address = Address::from_slice(account_address);
                Some(address)
            }
            _ => None,
        }
    }
}

impl ChainInspector for EvmChain {
    fn get_info(&self) -> ChainInfo {
        self.info.clone()
    }
}

impl ChainMutate for EvmChain {
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

impl BalanceFetcher for EvmChain {
    fn balance_of(
        &self,
        asset: AssetId,
        account: MultiLocation,
    ) -> core::result::Result<u128, RegistryError> {
        let transport = Eth::new(PinkHttp::new(String::from_utf8_lossy(&self.info.endpoint)));
        let token_address: Address = self
            .extract_token(&asset)
            .ok_or(RegistryError::ExtractLocationFailed)?;
        let account: Address = self
            .extract_account(&account)
            .ok_or(RegistryError::ExtractLocationFailed)?;
        let erc20 = Contract::from_json(
            transport,
            // PHA address
            token_address,
            include_bytes!("./res/erc20-abi.json"),
        )
        .map_err(|_| RegistryError::ConstructContractFailed)?;
        // TODO.wf handle potential failure smoothly instead of unwrap directly
        let result: u128 =
            resolve_ready(erc20.query("balanceOf", account, None, Options::default(), None))
                .unwrap();
        Ok(result)
    }
}

impl AssetsRegisry for EvmChain {
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
