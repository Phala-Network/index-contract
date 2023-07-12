//#[allow(clippy::large_enum_variant)]
use crate::chain::Chain;
use alloc::{string::String, vec::Vec};
use ink::storage::traits::StorageLayout;

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct Asset {
    pub id: u32,
    pub symbol: String,
    pub name: String,
    pub location: String,
    pub decimals: u32,
    pub chain_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct Dex {
    pub id: u32,
    pub name: String,
    pub chain_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct DexIndexer {
    pub id: u32,
    pub url: String,
    pub dex_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct DexPair {
    pub id: u32,
    pub asset0_id: u32,
    pub asset1_id: u32,
    pub dex_id: u32,
    pub pair_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct Bridge {
    pub id: u32,
    pub name: String,
    pub location: String,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct BridgePair {
    pub id: u32,
    pub asset0_id: u32,
    pub asset1_id: u32,
    pub bridge_id: u32,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct Graph {
    pub chains: Vec<Chain>,
    pub assets: Vec<Asset>,
    pub dexs: Vec<Dex>,
    pub dex_pairs: Vec<DexPair>,
    pub dex_indexers: Vec<DexIndexer>,
    pub bridges: Vec<Bridge>,
    pub bridge_pairs: Vec<BridgePair>,
}

impl Graph {
    pub fn get_chain(&self, name: String) -> Option<Chain> {
        let chains = &self.chains;
        chains
            .iter()
            .position(|c| c.name == name)
            .map(|idx| chains[idx].clone())
    }
}
