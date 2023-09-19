//#[allow(clippy::large_enum_variant)]
use crate::{
    call::CallBuilder,
    chain::{Chain, ChainType, ForeignAssetModule},
};
use ink::storage::traits::StorageLayout;

use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec,
    vec::Vec,
};

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
                    endpoint: "https://mainnet.infura.io/v3/e5f4c95222934613bbde028ba5dc526b"
                        .to_string(),
                    chain_type: ChainType::Evm,
                    native_asset: hex::decode("0000000000000000000000000000000000000000")
                        .expect("InvalidLocation"),
                    foreign_asset: None,
                    handler_contract: hex::decode("d693bDC5cb0cF2a31F08744A0Ec135a68C26FE1c")
                        .expect("InvalidLocation"),
                    tx_indexer: "https://squid.subsquid.io/graph-ethereum/graphql".to_string(),
                },
                Chain {
                    id: 1,
                    name: "Moonbeam".to_string(),
                    endpoint: "https://moonbeam.api.onfinality.io/public".to_string(),
                    chain_type: ChainType::Evm,
                    native_asset: hex::decode("0000000000000000000000000000000000000802")
                        .expect("InvalidLocation"),
                    foreign_asset: None,
                    handler_contract: hex::decode("B8D20dfb8c3006AA17579887ABF719DA8bDf005B")
                        .expect("InvalidLocation"),
                    tx_indexer: "https://squid.subsquid.io/graph-moonbeam/graphql".to_string(),
                },
                Chain {
                    id: 2,
                    name: "AstarEvm".to_string(),
                    endpoint: "https://astar.public.blastapi.io".to_string(),
                    chain_type: ChainType::Evm,
                    native_asset: hex::decode("0000000000000000000000000000000000000000")
                        .expect("InvalidLocation"),
                    foreign_asset: None,
                    handler_contract: hex::decode("bEA1C40ecf9c4603ec25264860B9b6623Ff733F5")
                        .expect("InvalidLocation"),
                    tx_indexer: "https://squid.subsquid.io/graph-astar/graphql".to_string(),
                },
                Chain {
                    id: 3,
                    name: "Astar".to_string(),
                    endpoint: "https://astar.public.blastapi.io".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("010100591f").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::PalletAsset),
                    // FIXME: Handle contract on AStar Sub
                    handler_contract: hex::decode("00").expect("InvalidLocation"),
                    tx_indexer: "https://squid.subsquid.io/graph-astar/graphql".to_string(),
                },
                Chain {
                    id: 4,
                    name: "Khala".to_string(),
                    endpoint: "https://khala-api.phala.network/rpc".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("0000").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::PalletAsset),
                    handler_contract: hex::decode("79").expect("InvalidLocation"),
                    tx_indexer: "https://squid.subsquid.io/graph-khala/graphql".to_string(),
                },
                Chain {
                    id: 5,
                    name: "Phala".to_string(),
                    endpoint: "https://api.phala.network/rpc".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("0000").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::PalletAsset),
                    handler_contract: hex::decode("79").expect("InvalidLocation"),
                    tx_indexer: "https://squid.subsquid.io/graph-phala/graphql".to_string(),
                },
                Chain {
                    id: 6,
                    name: "Acala".to_string(),
                    endpoint: "https://acala-rpc.dwellir.com".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("010200411f06080000").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::OrmlToken),
                    // FIXME: No Handler pallet in Acala
                    handler_contract: hex::decode("00").expect("InvalidLocation"),
                    tx_indexer: "https://squid.subsquid.io/graph-acala/graphql".to_string(),
                },
            ],
        }
    }

    pub fn get_chain(&self, name: &String) -> Option<Chain> {
        let chains = &self.chains;
        chains
            .iter()
            .position(|c| &c.name == name)
            .map(|idx| chains[idx].clone())
    }

    pub fn create_actions(&self, chain: &String) -> Vec<(String, Box<dyn CallBuilder>)> {
        let chain = self.get_chain(chain).expect("ChainNotFound");

        match chain.name.as_str() {
            "Acala" => crate::actions::acala::create_actions(&chain),
            "AstarEvm" => crate::actions::astar::evm_create_actions(&chain),
            "Astar" => crate::actions::astar::sub_create_actions(&chain),
            "Ethereum" => crate::actions::ethereum::create_actions(&chain),
            "Moonbeam" => crate::actions::moonbeam::create_actions(&chain),
            "Phala" | "Khala" => crate::actions::phala::create_actions(&chain),
            _ => vec![],
        }
    }
}
