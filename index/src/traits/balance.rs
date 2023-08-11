use pink_web3::{
    api::Namespace,
    contract::{Contract, Options},
    ethabi::Address,
    transports::{resolve_ready, PinkHttp},
    Eth,
};
use xcm::v2::{
    AssetId,
    AssetId::Concrete,
    Junction::{GeneralKey, Parachain},
    Junctions, MultiLocation,
};

use crate::prelude::Error;
use alloc::string::String;

/// Query the account balance of an asset under a multichain scenario is a mess,
/// not only because different chains have different account systems but also have
/// different asset registry mechanism(e.g. Acala use Currency, Phala use pallet-assets
/// manage registered foreign assets). Besides, query the native asset and foreign assets
/// on a chain also different
///
/// Use `AssetId` and `MultiLocation` to represent indentification of the `asset` and `account` respectively
/// is a good choice because developers can customize the way how they represent the `asset`
/// `account`. For example, for `USDC` on Ethereum, bridge1 can represent it with
/// `MultiLocation::new(1, X2(GeneralKey('Ethereum'), GeneralKey(usdc_addr))`, bridge2 can represent
/// it with `MultiLocation::new(1, X3(Parachain(2004), GeneralIndex(0), GeneralKey(usdc_addr))`.
///
/// Both `AssetId` and `MultiLocation` are primitives introduced by XCM format.
pub trait BalanceFetcher {
    /// Return on-chain `asset` amount of `account`
    fn balance_of(
        &self,
        asset: AssetId,
        account: MultiLocation,
    ) -> core::result::Result<u128, Error>;
}

pub struct EvmBalance {
    endpoint: String,
}

#[allow(dead_code)]
impl EvmBalance {
    pub fn new(endpoint: String) -> Self {
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
    ) -> core::result::Result<u128, Error> {
        let transport = Eth::new(PinkHttp::new(self.endpoint.clone()));
        let token_address: Address = self
            .extract_token(&asset)
            .ok_or(Error::ExtractLocationFailed)?;
        let account: Address = self
            .extract_account(&account)
            .ok_or(Error::ExtractLocationFailed)?;
        let erc20 = Contract::from_json(
            transport,
            // PHA address
            token_address,
            include_bytes!("../../../index/src/abis/erc20-abi.json"),
        )
        .map_err(|_| Error::ConstructContractFailed)?;
        // TODO.wf handle potential failure smoothly instead of unwrap directly
        let result: u128 =
            resolve_ready(erc20.query("balanceOf", account, None, Options::default(), None))
                .unwrap();
        Ok(result)
    }
}

// BalanceFetcher implementation for chain use pallet-assets as assets registry.
// See https://github.com/paritytech/substrate/tree/master/frame/assets
#[allow(dead_code)]
pub struct SubAssetsBalance {
    _endpoint: String,
}

impl SubAssetsBalance {
    pub fn _new(_endpoint: String) -> Self {
        SubAssetsBalance { _endpoint }
    }
}

impl BalanceFetcher for SubAssetsBalance {
    fn balance_of(
        &self,
        _asset: AssetId,
        _account: MultiLocation,
    ) -> core::result::Result<u128, Error> {
        // TODO.wf
        Err(Error::Unimplemented)
    }
}

// BalanceFetcher implementation for chain use currency as assets registry.
// See https://github.com/open-web3-stack/open-runtime-module-library/tree/master/currencies
#[allow(dead_code)]
pub struct SubCurrencyBalance {
    _endpoint: String,
}

impl SubCurrencyBalance {
    pub fn _new(_endpoint: String) -> Self {
        SubCurrencyBalance { _endpoint }
    }
}

impl BalanceFetcher for SubCurrencyBalance {
    fn balance_of(
        &self,
        _asset: AssetId,
        _account: MultiLocation,
    ) -> core::result::Result<u128, Error> {
        // TODO.wf
        Err(Error::Unimplemented)
    }
}
