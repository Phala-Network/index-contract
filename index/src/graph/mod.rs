mod asset;
mod bridge;
mod chain;
mod dex;

use scale::{Decode, Encode};

pub use self::{
    asset::Asset,
    bridge::{Bridge, BridgePair},
    chain::{Chain, ChainType, NonceFetcher},
    dex::{Dex, DexIndexer, DexPair},
};

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

impl Graph {
    pub fn get_chain(&self, name: String) -> Option<Chain> {
        let chains = &self.registered_chains;
        let chain = chains
            .iter()
            .position(|c| c.name == name)
            .map(|idx| chains[idx].clone());
        return chain;
    }
}
