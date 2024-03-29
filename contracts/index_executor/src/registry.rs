//#[allow(clippy::large_enum_variant)]
use crate::actions::ActionExtraInfo;
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
pub struct Asset {
    pub chain: String,
    pub symbol: String,
    pub location: Vec<u8>,
    pub decimals: u8,
}

#[derive(Clone, scale::Encode, scale::Decode, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct Registry {
    pub chains: Vec<Chain>,
    pub assets: Vec<Asset>,
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
                    endpoint: "https://rpc.ankr.com/eth".to_string(),
                    chain_type: ChainType::Evm,
                    native_asset: hex_literal::hex!("0000000000000000000000000000000000000000")
                        .to_vec(),
                    foreign_asset: None,
                    handler_contract: hex::decode("d693bDC5cb0cF2a31F08744A0Ec135a68C26FE1c")
                        .expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-ethereum/graphql".to_string(),
                },
                Chain {
                    id: 1,
                    name: "Moonbeam".to_string(),
                    endpoint: "https://rpc.api.moonbeam.network".to_string(),
                    chain_type: ChainType::Evm,
                    native_asset: hex::decode("0000000000000000000000000000000000000802")
                        .expect("InvalidLocation"),
                    foreign_asset: None,
                    handler_contract: hex::decode("8351BAE38E3D590063544A99A95BF4fe5379110b")
                        .expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-moonbeam/graphql".to_string(),
                },
                Chain {
                    id: 2,
                    name: "AstarEvm".to_string(),
                    endpoint: "https://astar.public.blastapi.io".to_string(),
                    chain_type: ChainType::Evm,
                    native_asset: hex_literal::hex!("0000000000000000000000000000000000000000")
                        .to_vec(),
                    foreign_asset: None,
                    handler_contract: hex::decode("AE1Ab0a83de66a545229d39E874237fbaFe05714")
                        .expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-astar/graphql".to_string(),
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
                    tx_indexer_url: "https://squid.subsquid.io/graph-astar/graphql".to_string(),
                },
                Chain {
                    id: 4,
                    name: "Khala".to_string(),
                    endpoint: "https://khala-api.phala.network/rpc".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("0000").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::PalletAsset),
                    handler_contract: hex::decode("79").expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-khala/graphql".to_string(),
                },
                Chain {
                    id: 5,
                    name: "Phala".to_string(),
                    endpoint: "https://api.phala.network/rpc".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("0000").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::PalletAsset),
                    handler_contract: hex::decode("79").expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-phala/graphql".to_string(),
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
                    tx_indexer_url: "https://squid.subsquid.io/graph-acala/graphql".to_string(),
                },
                Chain {
                    id: 7,
                    name: "Polkadot".to_string(),
                    endpoint: "https://polkadot.api.onfinality.io/public".to_string(),
                    chain_type: ChainType::Sub,
                    native_asset: hex::decode("0000").expect("InvalidLocation"),
                    foreign_asset: Some(ForeignAssetModule::PalletAsset),
                    // FIXME: No Handler pallet in Polkadot
                    handler_contract: hex::decode("00").expect("InvalidLocation"),
                    tx_indexer_url: "https://squid.subsquid.io/graph-polkadot/graphql".to_string(),
                },
            ],
            assets: vec![
                Asset {
                    chain: "Ethereum".to_string(),
                    symbol: "ETH".to_string(),
                    location: hex::decode("0000000000000000000000000000000000000000")
                        .expect("InvalidLocation"),
                    decimals: 18,
                },
                Asset {
                    chain: "Ethereum".to_string(),
                    symbol: "PHA".to_string(),
                    location: hex::decode("6c5bA91642F10282b576d91922Ae6448C9d52f4E")
                        .expect("InvalidLocation"),
                    decimals: 18,
                },
                Asset {
                    chain: "Phala".to_string(),
                    symbol: "PHA".to_string(),
                    location: hex::decode("0000").expect("InvalidLocation"),
                    decimals: 12,
                },
                Asset {
                    chain: "Khala".to_string(),
                    symbol: "PHA".to_string(),
                    location: hex::decode("0000").expect("InvalidLocation"),
                    decimals: 12,
                },
                Asset {
                    chain: "Moonbeam".to_string(),
                    symbol: "GLMR".to_string(),
                    location: hex::decode("0000000000000000000000000000000000000802")
                        .expect("InvalidLocation"),
                    decimals: 18,
                },
                Asset {
                    chain: "AstarEvm".to_string(),
                    symbol: "ASTR".to_string(),
                    location: [0; 20].to_vec(),
                    decimals: 18,
                },
                Asset {
                    chain: "AstarEvm".to_string(),
                    symbol: "GLMR".to_string(),
                    location: hex::decode("FFFFFFFF00000000000000010000000000000003")
                        .expect("InvalidLocation"),
                    decimals: 18,
                },
                Asset {
                    chain: "Astar".to_string(),
                    symbol: "ASTR".to_string(),
                    location: hex::decode("010100591f").expect("InvalidLocation"),
                    decimals: 18,
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

    pub fn get_asset(&self, chain: &String, location: &Vec<u8>) -> Option<Asset> {
        let assets = &self.assets;
        assets
            .iter()
            .position(|c| &c.chain == chain && &c.location == location)
            .map(|idx| assets[idx].clone())
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
            "Polkadot" => crate::actions::polkadot::create_actions(&chain),
            _ => vec![],
        }
    }

    pub fn get_action_extra_info(&self, chain: &str, action: &str) -> Option<ActionExtraInfo> {
        match chain {
            "Acala" => crate::actions::acala::get_extra_info(chain, action),
            "AstarEvm" => crate::actions::astar::get_extra_info(chain, action),
            "Astar" => crate::actions::astar::get_extra_info(chain, action),
            "Ethereum" => crate::actions::ethereum::get_extra_info(chain, action),
            "Moonbeam" => crate::actions::moonbeam::get_extra_info(chain, action),
            "Phala" | "Khala" => crate::actions::phala::get_extra_info(chain, action),
            _ => None,
        }
    }
}
