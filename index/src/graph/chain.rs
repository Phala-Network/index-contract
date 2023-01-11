use crate::prelude::Error;
use alloc::string::String;
use alloc::vec::Vec;
use pink_web3::{
    api::{Eth, Namespace},
    contract::{Contract, Options},
    transports::{resolve_ready, PinkHttp},
    types::Address,
    Web3,
};

use super::constants::*;
use pink_subrpc::{get_next_nonce, get_ss58addr_version, Ss58Codec};
use primitive_types::U256;

#[derive(Clone, Debug, Default, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum ChainType {
    #[default]
    Evm,
    Sub,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum ForeignAssetModule {
    #[default]
    PalletAsset,
    PalletCurrency,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Chain {
    pub id: u32,
    pub name: String,
    pub endpoint: String,
    pub chain_type: ChainType,
    // Encoded native asset location for Sub-chains
    pub native_asset: Vec<u8>,
    pub foreign_asset: Option<ForeignAssetModule>,
}

impl Chain {
    pub fn is_native(&self, asset: &Vec<u8>) -> bool {
        match self.chain_type {
            ChainType::Evm => {
                // A little bit tricky here
                asset == &vec![0]
            }
            ChainType::Sub => asset == &self.native_asset,
        }
    }
}

/// Query on-chain `account` nonce
pub trait NonceFetcher {
    fn get_nonce(&self, account: Vec<u8>) -> core::result::Result<u64, Error>;
}
impl NonceFetcher for Chain {
    fn get_nonce(&self, account: Vec<u8>) -> core::result::Result<u64, Error> {
        Ok(match self.chain_type {
            ChainType::Evm => {
                let account20: [u8; 20] = account.try_into().map_err(|_| Error::InvalidAddress)?;
                let evm_account: Address = account20.into();
                let eth = Eth::new(PinkHttp::new(self.endpoint.clone()));
                let nonce = resolve_ready(eth.transaction_count(evm_account, None))
                    .map_err(|_| Error::FetchDataFailed)?;
                nonce.try_into().expect("Nonce onverflow")
            }
            ChainType::Sub => {
                let version = get_ss58addr_version(&self.name).map_err(|_| Error::Ss58)?;
                let public_key: [u8; 32] = account.try_into().map_err(|_| Error::InvalidAddress)?;
                let addr = public_key.to_ss58check_with_version(version.prefix());
                get_next_nonce(&self.endpoint, &addr).map_err(|_| Error::FetchDataFailed)?
            }
        })
    }
}

/// Query on-chain account balance of an asset
pub trait BalanceFetcher {
    fn get_balance(&self, asset: Vec<u8>, account: Vec<u8>) -> core::result::Result<u128, Error>;
}

impl BalanceFetcher for Chain {
    fn get_balance(&self, asset: Vec<u8>, account: Vec<u8>) -> core::result::Result<u128, Error> {
        Ok(match self.chain_type {
            ChainType::Evm => {
                let transport = PinkHttp::new(&self.endpoint);
                let account20: [u8; 20] = account.try_into().map_err(|_| Error::InvalidAddress)?;
                let evm_account: Address = account20.into();

                if self.is_native(&asset) {
                    let web3 = Web3::new(transport);
                    let balance = resolve_ready(web3.eth().balance(evm_account, None))
                        .map_err(|_| Error::FetchDataFailed)?;
                    balance.try_into().expect("Balance onverflow")
                } else {
                    let eth = Eth::new(transport);
                    let asset_account20: [u8; 20] =
                        asset.try_into().map_err(|_| Error::InvalidAddress)?;
                    let token_address: Address = asset_account20.into();
                    let token = Contract::from_json(
                        eth,
                        token_address,
                        include_bytes!("../abis/erc20-abi.json"),
                    )
                    .expect("Bad abi data");
                    let balance: U256 = resolve_ready(token.query(
                        "balanceOf",
                        evm_account,
                        None,
                        Options::default(),
                        None,
                    ))
                    .map_err(|_| Error::FetchDataFailed)?;
                    balance.try_into().expect("Balance onverflow")
                }
            }
            ChainType::Sub => {
                if self.is_native(&asset) {
                    return Err(Error::Unimplemented);
                } else {
                    return Err(Error::Unimplemented);
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use hex_literal::hex;
    use ink_lang as ink;
    use scale::Encode;
    use xcm::v1::{prelude::*, MultiLocation};

    #[ink::test]
    fn test_get_evm_account_nonce() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let goerli = Chain {
            id: 1,
            name: String::from("Goerli"),
            endpoint: String::from(
                "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
            ),
            chain_type: ChainType::Evm,
            native_asset: vec![0],
            foreign_asset: None,
        };
        assert_eq!(
            goerli
                .get_nonce(hex!("0E275F8839b788B2674935AD97C01cF73A9E8c41").into())
                .unwrap(),
            2
        );
    }

    #[ink::test]
    fn test_get_sub_account_nonce() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let khala = Chain {
            id: 1,
            name: String::from("Khala"),
            endpoint: String::from("https://khala.api.onfinality.io:443/public-ws"),
            chain_type: ChainType::Sub,
            native_asset: MultiLocation::new(1, X1(Parachain(2035))).encode(),
            foreign_asset: Some(ForeignAssetModule::PalletAsset),
        };
        assert_eq!(
            khala
                .get_nonce(
                    hex!("92436be04f9dc677f9f51b092161b6e5ba00163ad6328fb2c920fcb30b6c7362").into()
                )
                .unwrap(),
            2
        );
    }

    #[ink::test]
    fn test_get_evm_account_balance() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let goerli = Chain {
            id: 1,
            name: String::from("Goerli"),
            endpoint: String::from(
                "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
            ),
            chain_type: ChainType::Evm,
            native_asset: vec![0],
            foreign_asset: None,
        };
        // Get native asset balance
        assert_eq!(
            goerli
                .get_balance(
                    hex!("00").into(),
                    hex!("0E275F8839b788B2674935AD97C01cF73A9E8c41").into()
                )
                .unwrap(),
            6_850_126_116_190_000u128
        );
        // Get GPHA balance
        assert_eq!(
            goerli
                .get_balance(
                    hex!("B376b0Ee6d8202721838e76376e81eEc0e2FE864").into(),
                    hex!("0E275F8839b788B2674935AD97C01cF73A9E8c41").into()
                )
                .unwrap(),
            5_000_000_000_000_000_000u128
        );
    }
}
