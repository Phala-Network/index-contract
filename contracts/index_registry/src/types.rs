extern crate alloc;

use alloc::{string::String, vec::Vec};
use ink_storage::traits::{PackedLayout, SpreadAllocate, SpreadLayout, StorageLayout};
use scale::{Decode, Encode};
use xcm::latest::{AssetId, MultiLocation};

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Error {
    BadAbi,
    BadOrigin,
    AssetAlreadyRegistered,
    AssetNotFound,
    BridgeAlreadyRegistered,
    BridgeNotFound,
    ChainAlreadyRegistered,
    ChainNotFound,
    DexAlreadyRegistered,
    DexNotFound,
    ExtractLocationFailed,
    ConstructContractFailed,
    Unimplemented,
}

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

/// Beyond general properties like `name`, `symbol` and `decimals`,
/// a `location` is needed to identify the asset between multi-chains
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    SpreadLayout,
    PackedLayout,
    SpreadAllocate,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct AssetInfo {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    /// Encoded asset MultiLocation
    pub location: Vec<u8>,
}

pub trait AssetsRegisry {
    /// Register the asset
    /// Authorized method, only the contract owner can do
    fn register(&mut self, asset: AssetInfo) -> core::result::Result<(), Error>;

    /// Unregister the asset
    /// Authorized method, only the contract owner can do
    fn unregister(&mut self, asset: AssetInfo) -> core::result::Result<(), Error>;

    /// Return all registerd assets
    fn registered_assets(&self) -> Vec<AssetInfo>;

    fn lookup_by_name(&self, name: String) -> Option<AssetInfo>;

    fn lookup_by_symbol(&self, symbol: String) -> Option<AssetInfo>;

    fn lookup_by_location(&self, location: Vec<u8>) -> Option<AssetInfo>;
}

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub enum ChainType {
    Evm,
    Sub,
}

#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct ChainInfo {
    pub name: String,
    pub chain_type: ChainType,
    pub native: Option<AssetInfo>,
    pub stable: Option<AssetInfo>,
    pub endpoint: String,
    pub network: Option<u8>,
}

/// Query on-chain `account` nonce
pub trait NonceFetcher {
    fn get_nonce(&self, account: Vec<u8>) -> core::result::Result<u64, Error>;
}
impl NonceFetcher for ChainInfo {
    fn get_nonce(&self, _account: Vec<u8>) -> core::result::Result<u64, Error> {
        Err(Error::Unimplemented)
    }
}

pub trait ChainInspector {
    /// Return information of the chain
    fn get_info(&self) -> ChainInfo;
}

pub trait ChainMutate {
    fn set_native(&mut self, native: AssetInfo);
    fn set_stable(&mut self, stable: AssetInfo);
    fn set_endpoint(&mut self, endpoint: String);
}

/// Asset informatios should be contained in the input graph
#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
pub struct AssetGraph {
    /// Chain name that asset belong to
    pub chain: String,
    /// Encoded asset MultiLocation
    pub location: Vec<u8>,
    /// Asset name
    pub name: String,
    /// Symbol of asset
    pub symbol: String,
    /// Decimal of asset
    pub decimals: u8,
}

/// Trading pair informatios should be contained in the input graph
#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
pub struct TradingPairGraph {
    /// Indentification of the trading pair on dex
    pub id: Vec<u8>,
    /// Name of asset0
    pub asset0: String,
    /// Name of asset1
    pub asset1: String,
    /// Dex name that trading pair belong to
    pub dex: String,
    /// Chain name that trading pair belong to
    pub chain: String,
}

/// Bridge informations should be contained in the input graph
#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
pub struct BridgeGraph {
    /// Name of source chain
    pub chain0: String,
    /// Name of dest chain
    pub chain1: String,
    /// Asset name of bridge pair.
    pub assets: Vec<(String, String)>,
}

/// Definition of the input graph
#[derive(Clone, Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
pub struct Graph {
    /// All registered assets
    pub assets: Vec<AssetGraph>,
    /// All registered trading pairs
    pub pairs: Vec<TradingPairGraph>,
    /// All supported bridges
    pub bridges: Vec<BridgeGraph>,
}
