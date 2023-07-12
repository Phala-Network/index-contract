//#[allow(clippy::large_enum_variant)]
use crate::chain::{Chain, ChainType, ForeignAssetModule};
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

    #[allow(clippy::type_complexity)]
    pub fn create_bridge_executors(&self) -> Vec<((String, String), Box<dyn BridgeExecutor>)> {
        let mut bridge_executors: Vec<((String, String), Box<dyn BridgeExecutor>)> = vec![];
        let moonbeam = self
            .get_chain(String::from("Moonbeam"))
            .expect("ChainNotFound");
        let phala = self
            .get_chain(String::from("Phala"))
            .expect("ChainNotFound");
        let khala = self
            .get_chain(String::from("Khala"))
            .expect("ChainNotFound");
        let ethereum = self
            .get_chain(String::from("Ethereum"))
            .expect("ChainNotFound");

        let moonbeam_xtoken: [u8; 20] =
            hex_literal::hex!("0000000000000000000000000000000000000804");
        let chainbridge_on_ethereum: [u8; 20] =
            hex_literal::hex!("8F92e7353b180937895E0C5937d616E8ea1A2Bb9");

        // Moonbeam -> Acala
        bridge_executors.push((
            (String::from("Moonbeam"), String::from("Acala")),
            Box::new(MoonbeamXTokenExecutor::new(
                &moonbeam.endpoint,
                moonbeam_xtoken.into(),
                ACALA_PARACHAIN_ID,
            )),
        ));
        // Moonbeam -> Phala
        bridge_executors.push((
            (String::from("Moonbeam"), String::from("Phala")),
            Box::new(MoonbeamXTokenExecutor::new(
                &moonbeam.endpoint,
                moonbeam_xtoken.into(),
                PHALA_PARACHAIN_ID,
            )),
        ));
        // Phala -> Acala
        bridge_executors.push((
            (String::from("Phala"), String::from("Acala")),
            Box::new(PhalaXTransferExecutor::new(
                &phala.endpoint,
                ACALA_PARACHAIN_ID,
                index::AccountType::Account32,
            )),
        ));
        // Ethereum -> Phala
        bridge_executors.push((
            (String::from("Ethereum"), String::from("Phala")),
            Box::new(ChainBridgeEthereum2Phala::new(
                &ethereum.endpoint,
                CHAINBRIDGE_ID_PHALA,
                chainbridge_on_ethereum.into(),
                vec![(
                    // PHA contract address on Ethereum
                    hex_literal::hex!("6c5bA91642F10282b576d91922Ae6448C9d52f4E").into(),
                    // PHA ChainBridge resource id on Phala
                    hex_literal::hex!(
                        "00b14e071ddad0b12be5aca6dffc5f2584ea158d9b0ce73e1437115e97a32a3e"
                    ),
                )],
            )),
        ));
        // Phala -> Ethereum
        bridge_executors.push((
            (String::from("Phala"), String::from("Ethereum")),
            Box::new(ChainBridgePhala2Ethereum::new(
                CHAINBRIDGE_ID_ETHEREUM,
                &phala.endpoint,
            )),
        ));
        // Ethereum -> Khala
        bridge_executors.push((
            (String::from("Ethereum"), String::from("Khala")),
            Box::new(ChainBridgeEthereum2Phala::new(
                &ethereum.endpoint,
                CHAINBRIDGE_ID_KHALA,
                chainbridge_on_ethereum.into(),
                vec![(
                    // PHA contract address on Ethereum
                    hex_literal::hex!("6c5bA91642F10282b576d91922Ae6448C9d52f4E").into(),
                    // PHA ChainBridge resource id on Khala
                    hex_literal::hex!(
                        "00e6dfb61a2fb903df487c401663825643bb825d41695e63df8af6162ab145a6"
                    ),
                )],
            )),
        ));
        // Khala -> Ethereum
        bridge_executors.push((
            (String::from("Khala"), String::from("Ethereum")),
            Box::new(ChainBridgePhala2Ethereum::new(
                CHAINBRIDGE_ID_ETHEREUM,
                &khala.endpoint,
            )),
        ));
        bridge_executors
    }

    pub fn create_dex_executors(&self) -> Vec<(String, Box<dyn DexExecutor>)> {
        let mut dex_executors: Vec<(String, Box<dyn DexExecutor>)> = vec![];
        let moonbeam = self
            .get_chain(String::from("Moonbeam"))
            .expect("ChainNotFound");
        let acala = self
            .get_chain(String::from("Acala"))
            .expect("ChainNotFound");

        let stellaswap_router: [u8; 20] = hex::decode("70085a09D30D6f8C4ecF6eE10120d1847383BB57")
            .unwrap()
            .to_array();

        // Acala DEX
        dex_executors.push((
            String::from("Acala"),
            Box::new(AcalaDexExecutor::new(&acala.endpoint)),
        ));
        // Moonbeam::StellaSwap
        dex_executors.push((
            String::from("Moonbeam"),
            Box::new(MoonbeamDexExecutor::new(
                &moonbeam.endpoint,
                stellaswap_router.into(),
            )),
        ));
        dex_executors
    }

    pub fn create_transfer_executors(&self) -> Vec<(String, Box<dyn TransferExecutor>)> {
        let mut transfer_executors: Vec<(String, Box<dyn TransferExecutor>)> = vec![];
        let acala = self
            .get_chain(String::from("Acala"))
            .expect("ChainNotFound");

        transfer_executors.push((
            String::from("Acala"),
            Box::new(AcalaTransferExecutor::new(&acala.endpoint)),
        ));
        transfer_executors
    }
}