mod account;
mod asset;
mod bridge;
mod chain;
mod dex;

use alloc::string::String;
use alloc::vec::Vec;
use scale::{Decode, Encode};

pub use self::{
    account::*,
    asset::Asset,
    bridge::{Bridge, BridgePair},
    chain::{Chain, ChainType, ForeignAssetModule, NonceFetcher},
    dex::{Dex, DexIndexer, DexPair},
};

#[derive(Clone, Default, Encode, Decode, Debug)]
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
