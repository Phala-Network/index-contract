extern crate alloc;

use super::chain_store::ChainStore;
use crate::traits::{
    common::Error as RegistryError,
    registry::{AssetInfo, AssetsRegisry, BalanceFetcher, ChainInfo, ChainInspector, ChainMutate},
};
use alloc::{string::String, vec::Vec};
use ink_storage::traits::{PackedLayout, SpreadLayout, StorageLayout};
use pink_web3::api::{Eth, Namespace};
use pink_web3::contract::{Contract, Options};
use pink_web3::transports::{resolve_ready, PinkHttp};
use pink_web3::types::Address;
use xcm::latest::{prelude::*, MultiLocation};

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct Chain {
    pub store: ChainStore,
}

impl Chain {
    /// Create an Chain entity
    pub fn new(info: ChainInfo) -> Self {
        Chain {
            store: ChainStore::new(info),
        }
    }
}

impl ChainInspector for Chain {
    fn get_info(&self) -> ChainInfo {
        self.store.get_info()
    }
}

impl ChainMutate for Chain {
    fn set_native(&mut self, native: AssetInfo) {
        self.store.set_native(native)
    }

    fn set_stable(&mut self, stable: AssetInfo) {
        self.store.set_stable(stable)
    }

    fn set_endpoint(&mut self, endpoint: Vec<u8>) {
        self.store.set_endpoint(endpoint)
    }
}

impl AssetsRegisry for Chain {
    /// Register the asset
    fn register(&mut self, asset: AssetInfo) -> core::result::Result<(), RegistryError> {
        self.store.register(asset)
    }

    /// Unregister the asset
    fn unregister(&mut self, asset: AssetInfo) -> core::result::Result<(), RegistryError> {
        self.store.unregister(asset)
    }

    /// Return all registerd assets
    fn registered_assets(&self) -> Vec<AssetInfo> {
        self.store.registered_assets()
    }

    fn lookup_by_name(&self, name: Vec<u8>) -> Option<AssetInfo> {
        self.store.lookup_by_name(name)
    }

    fn lookup_by_symbol(&self, symbol: Vec<u8>) -> Option<AssetInfo> {
        self.store.lookup_by_symbol(symbol)
    }

    fn lookup_by_location(&self, location: Vec<u8>) -> Option<AssetInfo> {
        self.store.lookup_by_location(location)
    }
}

pub struct EvmBalance {
    endpoint: Vec<u8>,
}

impl EvmBalance {
    pub fn new(endpoint: Vec<u8>) -> Self {
        EvmBalance { endpoint }
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

impl BalanceFetcher for EvmBalance {
    fn balance_of(
        &self,
        asset: AssetId,
        account: MultiLocation,
    ) -> core::result::Result<u128, RegistryError> {
        let transport = Eth::new(PinkHttp::new(String::from_utf8_lossy(&self.endpoint)));
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

// BalanceFetcher implementation for chain use pallet-assets as assets registry.
// See https://github.com/paritytech/substrate/tree/master/frame/assets
#[warn(dead_code)]
pub struct SubAssetsBalance {
    _endpoint: Vec<u8>,
}

impl SubAssetsBalance {
    pub fn new(_endpoint: Vec<u8>) -> Self {
        SubAssetsBalance { _endpoint }
    }
}

impl BalanceFetcher for SubAssetsBalance {
    fn balance_of(
        &self,
        _asset: AssetId,
        _account: MultiLocation,
    ) -> core::result::Result<u128, RegistryError> {
        // TODO.wf
        Err(RegistryError::Unimplemented)
    }
}

// BalanceFetcher implementation for chain use currency as assets registry.
// See https://github.com/open-web3-stack/open-runtime-module-library/tree/master/currencies
#[warn(dead_code)]
pub struct SubCurrencyBalance {
    _endpoint: Vec<u8>,
}

impl SubCurrencyBalance {
    pub fn new(_endpoint: Vec<u8>) -> Self {
        SubCurrencyBalance { _endpoint }
    }
}

impl BalanceFetcher for SubCurrencyBalance {
    fn balance_of(
        &self,
        _asset: AssetId,
        _account: MultiLocation,
    ) -> core::result::Result<u128, RegistryError> {
        // TODO.wf
        Err(RegistryError::Unimplemented)
    }
}
