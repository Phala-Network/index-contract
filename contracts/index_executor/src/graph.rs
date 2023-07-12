//#[allow(clippy::large_enum_variant)]::chain::Chain;
use alloc::{string::String, vec::Vec};
use ink::storage::traits::StorageLayout;

use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use index::prelude::AcalaDexExecutor;
use index::prelude::*;
use index::traits::executor::TransferExecutor;
use index::utils::ToArray;

#[derive(Clone, scale::Encode, scale::Decode, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct Registry {
    pub chains: Vec<Chain>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            chains: vec![
                Chain {
                    id: 0,
                    name: "Ethereum".to_string(),
                    endpoint: "https://mainnet.infura.io/v3/6d61e7957c1c489ea8141e947447405b"
                        .to_string(),
                    chain_type: ChainType::Evm,
                    native_asset: hex::decode("0000000000000000000000000000000000000000")
                        .expect("InvalidLocation"),
                    foreign_asset: None,
                    handler_contract: hex::decode("F9eaE3Ec6BFE94F510eb3a5de8Ac9dEB9E74DF39")
                        .expect("InvalidLocation"),
                    tx_indexer_url: "null".to_string(),
                },
                Chain {
                    id: 1,
                    name: "Moonbeam".to_string(),
                    endpoint: "https://moonbeam.api.onfinality.io/public".to_string(),
                    chain_type: ChainType::Evm,
                    native_asset: hex::decode("0000000000000000000000000000000000000000")
                        .expect("InvalidLocation"),
                    foreign_asset: None,
                    handler_contract: hex::decode("1e4ED6d37685D2FB254e47C5b58Cf95173326E4c")
                        .expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-moonbeam/graphql".to_string(),
                },
                Chain {
                    id: 2,
                    name: "Astar".to_string(),
                    endpoint: "https://astar.public.blastapi.io".to_string(),
                    chain_type: ChainType::Evm,
                    native_asset: hex::decode("0000000000000000000000000000000000000000")
                        .expect("InvalidLocation"),
                    foreign_asset: None,
                    // FIXME: Handle contract on AStar
                    handler_contract: hex::decode("0000000000000000000000000000000000000000")
                        .expect("InvalidLocation"),
                    tx_indexer_url: "null".to_string(),
                },
                Chain {
                    id: 3,
                    name: "Khala".to_string(),
                    endpoint: "wss://khala-api.phala.network/ws".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("0000").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::PalletAsset),
                    handler_contract: hex::decode("79").expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-khala/graphql".to_string(),
                },
                Chain {
                    id: 4,
                    name: "Phala".to_string(),
                    endpoint: "https://api.phala.network/rpc".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("0000").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::PalletAsset),
                    handler_contract: hex::decode("79").expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-phala/graphql".to_string(),
                },
                Chain {
                    id: 5,
                    name: "Acala".to_string(),
                    endpoint: "https://acala-rpc.dwellir.com".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("010200411f06080000").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::OrmlToken),
                    // FIXME: No Handler pallet in Acala
                    handler_contract: hex::decode("00").expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-acala/graphql".to_string(),
                },
            ],
        }
    }

    pub fn get_chain(&self, name: String) -> Option<Chain> {
        let chains = &self.chains;
        chains
            .iter()
            .position(|c| c.name == name)
            .map(|idx| chains[idx].clone())
    }
}
