//#[allow(clippy::large_enum_variant)]
use alloc::{string::String, vec::Vec};
use index::graph as index_graph;
use ink::storage::traits::StorageLayout;

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct Chain {
    pub id: u32,
    pub name: String,
    pub endpoint: String,
    pub chain_type: u32,
    pub native_asset: u32,
    pub foreign_asset_type: u32,
    pub handler_contract: String,
}

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

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
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

impl TryInto<index_graph::Graph> for Graph {
    type Error = &'static str;
    fn try_into(self) -> core::result::Result<index_graph::Graph, Self::Error> {
        let mut local_graph: index_graph::Graph = index_graph::Graph::default();

        {
            let mut arr: Vec<index_graph::Chain> = Vec::new();
            for chain in &self.chains {
                let item: index_graph::Chain = index_graph::Chain {
                    id: chain.id,
                    name: chain.name.clone(),
                    endpoint: chain.endpoint.clone(),
                    chain_type: {
                        match chain.chain_type {
                            // 0 => index_graph::ChainType::Unknown,
                            1 => index_graph::ChainType::Evm,
                            2 => index_graph::ChainType::Sub,
                            _ => return Err("Unsupported chain!"),
                        }
                    },
                    native_asset: {
                        let asset_id = chain.native_asset;
                        let asset = &self.assets[asset_id as usize - 1];
                        hexified_to_vec_u8(&asset.location).or(Err("InvalidInput"))?
                    },
                    foreign_asset: {
                        match chain.foreign_asset_type {
                            1 => Some(index_graph::ForeignAssetModule::PalletAsset),
                            2 => Some(index_graph::ForeignAssetModule::OrmlToken),
                            _ => return Err("Unsupported chain!"),
                        }
                    },
                    handler_contract: hexified_to_vec_u8(&chain.handler_contract)
                        .or(Err("InvalidInput"))?,
                };
                arr.push(item);
            }
            local_graph.chains = arr;
        }

        {
            let mut arr: Vec<index_graph::Asset> = Vec::new();
            for asset in &self.assets {
                let item = index_graph::Asset {
                    id: asset.id,
                    symbol: asset.symbol.clone(),
                    name: asset.name.clone(),
                    location: hexified_to_vec_u8(&asset.location).or(Err("InvalidInput"))?,
                    decimals: asset.decimals,
                    chain_id: asset.chain_id,
                };
                arr.push(item);
            }
            local_graph.assets = arr;
        }

        {
            let mut arr = Vec::new();
            for dex in &self.dexs {
                let item = index_graph::Dex {
                    id: dex.id,
                    name: dex.name.clone(),
                    chain_id: dex.chain_id,
                };
                arr.push(item);
            }
            local_graph.dexs = arr;
        }

        {
            let mut arr = Vec::new();
            for indexer in &self.dex_indexers {
                let item = index_graph::DexIndexer {
                    id: indexer.id,
                    url: indexer.url.clone(),
                    dex_id: indexer.dex_id,
                };
                arr.push(item);
            }
            local_graph.dex_indexers = arr;
        }

        {
            let mut arr = Vec::new();
            for pair in &self.dex_pairs {
                let item = index_graph::DexPair {
                    id: pair.id,
                    asset0_id: pair.asset0_id,
                    asset1_id: pair.asset1_id,
                    dex_id: pair.dex_id,
                    pair_id: hexified_to_vec_u8(&pair.pair_id).or(Err("InvalidInput"))?,
                };
                arr.push(item);
            }
            local_graph.dex_pairs = arr;
        }

        {
            let mut arr = Vec::new();
            for bridge in &self.bridges {
                let item = index_graph::Bridge {
                    id: bridge.id,
                    name: bridge.name.clone(),
                    location: hexified_to_vec_u8(&bridge.location).or(Err("InvalidInput"))?,
                };
                arr.push(item);
            }
            local_graph.bridges = arr;
        }

        {
            let mut arr = Vec::new();
            for pair in &self.bridge_pairs {
                let item = index_graph::BridgePair {
                    id: pair.id,
                    asset0_id: pair.asset0_id,
                    asset1_id: pair.asset1_id,
                    bridge_id: pair.bridge_id,
                };
                arr.push(item);
            }
            local_graph.bridge_pairs = arr;
        }

        Ok(local_graph)
    }
}

impl From<index_graph::Graph> for Graph {
    fn from(graph: index_graph::Graph) -> Graph {
        let mut local_graph: Graph = Graph::default();

        {
            let mut arr: Vec<Chain> = Vec::new();
            for chain in &graph.chains {
                let item: Chain = Chain {
                    id: chain.id,
                    name: chain.name.clone(),
                    endpoint: chain.endpoint.clone(),
                    chain_type: {
                        match chain.chain_type {
                            index_graph::ChainType::Evm => 1,
                            index_graph::ChainType::Sub => 2,
                        }
                    },
                    native_asset: {
                        let location = &chain.native_asset;
                        let asset = graph
                            .assets
                            .iter()
                            .find(|a| a.chain_id == chain.id && &a.location == location)
                            .expect("must not fail");
                        asset.id
                    },
                    foreign_asset_type: {
                        match chain.foreign_asset {
                            Some(index_graph::ForeignAssetModule::PalletAsset) => 1,
                            Some(index_graph::ForeignAssetModule::OrmlToken) => 2,
                            // FIXME: Is is reasonable here
                            None => 3,
                        }
                    },
                    handler_contract: hex::encode(chain.handler_contract.clone()),
                };
                arr.push(item);
            }
            local_graph.chains = arr;
        }

        {
            let mut arr: Vec<Asset> = Vec::new();
            for asset in &graph.assets {
                let item = Asset {
                    id: asset.id,
                    symbol: asset.symbol.clone(),
                    name: asset.name.clone(),
                    location: hex::encode(asset.location.clone()),
                    decimals: asset.decimals,
                    chain_id: asset.chain_id,
                };
                arr.push(item);
            }
            local_graph.assets = arr;
        }

        {
            let mut arr = Vec::new();
            for dex in &graph.dexs {
                let item = Dex {
                    id: dex.id,
                    name: dex.name.clone(),
                    chain_id: dex.chain_id,
                };
                arr.push(item);
            }
            local_graph.dexs = arr;
        }

        {
            let mut arr = Vec::new();
            for indexer in &graph.dex_indexers {
                let item = DexIndexer {
                    id: indexer.id,
                    url: indexer.url.clone(),
                    dex_id: indexer.dex_id,
                };
                arr.push(item);
            }
            local_graph.dex_indexers = arr;
        }

        {
            let mut arr = Vec::new();
            for pair in &graph.dex_pairs {
                let item = DexPair {
                    id: pair.id,
                    asset0_id: pair.asset0_id,
                    asset1_id: pair.asset1_id,
                    dex_id: pair.dex_id,
                    pair_id: hex::encode(pair.pair_id.clone()),
                };
                arr.push(item);
            }
            local_graph.dex_pairs = arr;
        }

        {
            let mut arr = Vec::new();
            for bridge in &graph.bridges {
                let item = Bridge {
                    id: bridge.id,
                    name: bridge.name.clone(),
                    location: hex::encode(bridge.location.clone()),
                };
                arr.push(item);
            }
            local_graph.bridges = arr;
        }

        {
            let mut arr = Vec::new();
            for pair in &graph.bridge_pairs {
                let item = BridgePair {
                    id: pair.id,
                    asset0_id: pair.asset0_id,
                    asset1_id: pair.asset1_id,
                    bridge_id: pair.bridge_id,
                };
                arr.push(item);
            }
            local_graph.bridge_pairs = arr;
        }

        local_graph
    }
}

// some field from the first graph(the RegistryGraph) is a String that is hexified somewhere else,
// the right way to decode it is:
//  - de-hexify it to be Vec<u8>
//  - restore the string from Vec<u8>
// for example:
// - a tool hexifies a string "0x3a62a4980b952C92f4d4243c4A009336Ee0a26eB" into 33613632613439383062393532433932663464343234336334413030393333364565306132366542
// - Phat contract receives 33613632613439383062393532433932663464343234336334413030393333364565306132366542
// - Phat contract needs to decode 33613632613439383062393532433932663464343234336334413030393333364565306132366542 into 0x3a62a4980b952C92f4d4243c4A009336Ee0a26eB
// - 0x3a62a4980b952C92f4d4243c4A009336Ee0a26eB is in bytes because the hex::decode gives Vec<u8> output
// - restore string from bytes using String::from_utf8_lossy
#[allow(dead_code)]
fn hexified_to_string(hs: &str) -> core::result::Result<String, &'static str> {
    Ok(String::from_utf8_lossy(&hex::decode(hs).or(Err("DecodeFailed"))?).to_string())
}

// when we restore a string from hexified string, to turn that into Vec<u8>,
// first thing is to remove the prefixing 0x, then hex::decode again
fn hexified_to_vec_u8(hs: &str) -> core::result::Result<Vec<u8>, &'static str> {
    let binding = hex::decode(hs).or(Err("DecodeFailed"))?;
    let headless = &String::from_utf8_lossy(&binding)[2..];
    Ok(hex::decode(headless).or(Err("DecodeFailed"))?)
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use primitive_types::H160;

    use super::*;

    #[test]
    fn string_codec_should_work() {
        let input =
            "307833613632613439383062393532433932663464343234336334413030393333364565306132366542"
                .to_string();
        assert_eq!(
            "0x3a62a4980b952C92f4d4243c4A009336Ee0a26eB".to_string(),
            hexified_to_string(&input).unwrap()
        );
        let v = hexified_to_vec_u8(&input).unwrap();
        assert_eq!(
            vec![
                0x3a, 0x62, 0xa4, 0x98, 0x0b, 0x95, 0x2C, 0x92, 0xf4, 0xd4, 0x24, 0x3c, 0x4A, 0x00,
                0x93, 0x36, 0xEe, 0x0a, 0x26, 0xeB
            ],
            v
        );
        let h1 = H160::from_slice(&v);
        let h2 = H160::from_str("0x3a62a4980b952C92f4d4243c4A009336Ee0a26eB").unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn graph_conversion_should_work() {
        // we are not registering entities manually!
        // just for demonstration.
        // there is a specfic management tool for all this data management
        let ethereum = Chain {
            id: 1,
            name: "Ethereum".to_string(),
            chain_type: 1,
            endpoint: "endpoint".to_string(),
            native_asset: 3,
            foreign_asset_type: 1,
            handler_contract: hex::encode("056C0E37d026f9639313C281250cA932C9dbe921"),
        };
        let phala = Chain {
            id: 2,
            name: "Phala".to_string(),
            chain_type: 2,
            endpoint: "endpoint".to_string(),
            native_asset: 2,
            foreign_asset_type: 1,
            handler_contract: hex::encode("056C0E37d026f9639313C281250cA932C9dbe921"),
        };
        let pha_on_ethereum = Asset {
            id: 1,
            chain_id: 1,
            name: "Phala Token".to_string(),
            symbol: "PHA".to_string(),
            decimals: 18,
            location: hex::encode("Somewhere on Ethereum"),
        };
        let pha_on_phala = Asset {
            id: 2,
            chain_id: 2,
            name: "Phala Token".to_string(),
            symbol: "PHA".to_string(),
            decimals: 12,
            location: hex::encode("Somewhere on Phala"),
        };
        let weth_on_ethereum = Asset {
            id: 3,
            chain_id: 1,
            name: "Wrap Ether".to_string(),
            symbol: "WETH".to_string(),
            decimals: 18,
            location: hex::encode("Somewhere on Ethereum2"),
        };
        let weth_on_phala = Asset {
            id: 4,
            chain_id: 2,
            name: "Phala Wrap Ether".to_string(),
            symbol: "pWETH".to_string(),
            decimals: 18,
            location: hex::encode("Somewhere on Phala2"),
        };
        let ethereum2phala_pha_pair = BridgePair {
            id: 1,
            asset0_id: 1,
            asset1_id: 2,
            bridge_id: 1,
        };
        let ethereum2phala_weth_pair = BridgePair {
            id: 2,
            asset0_id: 3,
            asset1_id: 4,
            bridge_id: 1,
        };
        let phala2ethereum_pha_pair = BridgePair {
            id: 3,
            asset0_id: 2,
            asset1_id: 1,
            bridge_id: 1,
        };
        let pha_weth_dex_pair = DexPair {
            id: 1,
            dex_id: 1,
            pair_id: hex::encode("pair_address"),
            asset0_id: 1,
            asset1_id: 3,
        };
        let bridge = Bridge {
            id: 1,
            name: "demo bridge".to_string(),
            location: hex::encode("xtoken://0x1213435"),
        };
        let dex = Dex {
            id: 1,
            name: "UniSwapV2".to_string(),
            chain_id: 1,
        };

        // should have a jonction table but this structure suffices
        let dex_indexer = DexIndexer {
            id: 1,
            url: "https://some-graph.network".to_string(),
            dex_id: 1,
        };

        let graph = Graph {
            chains: vec![ethereum, phala],
            assets: vec![
                pha_on_ethereum,
                pha_on_phala,
                weth_on_ethereum,
                weth_on_phala,
            ],
            dexs: vec![dex],
            bridges: vec![bridge],
            dex_pairs: vec![pha_weth_dex_pair],
            bridge_pairs: vec![
                ethereum2phala_pha_pair,
                ethereum2phala_weth_pair,
                phala2ethereum_pha_pair,
            ],
            dex_indexers: vec![dex_indexer],
        };

        let index_graph: index_graph::Graph = graph.clone().try_into().unwrap();
        let decoded_graph: Graph = index_graph.into();
        assert_eq!(decoded_graph, graph)
    }
}
