use core::iter::Map;

use scale::{Decode, Encode};

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Chain {
    pub id: u32,
    pub name: String,
    pub endpoint: String,
    pub chain_type: u32,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Asset {
    pub id: u32,
    pub symbol: String,
    pub name: String,
    // of type MultiLocation
    pub location: Vec<u8>,
    pub decimals: u32,
    pub chain_id: u32,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Dex {
    pub id: u32,
    pub name: String,
    pub chain_id: u32,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct DexIndexer {
    pub id: u32,
    pub url: String,
    pub dex_id: u32,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct DexPair {
    pub id: u32,
    pub asset0_id: u32,
    pub asset1_id: u32,
    pub dex_id: u32,
    pub pair_id: Vec<u8>,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Bridge {
    pub id: u32,
    pub name: String,
    pub location: Vec<u8>,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct BridgePair {
    pub id: u32,
    pub asset0_id: u32,
    pub asset1_id: u32,
    pub bridge_id: u32,
}

#[derive(Clone, Default, Encode, Decode, Debug)]
pub struct Graph {
    pub registered_chains: Vec<Chain>,
    pub registered_assets: Vec<Asset>,
    pub registered_dexs: Vec<Dex>,
    pub registered_dex_pairs: Vec<DexPair>,
    pub registered_dex_indexers: Vec<DexIndexer>,
    pub registered_bridges: Vec<Bridge>,
    pub registered_bridge_pairs: Vec<BridgePair>,
}
