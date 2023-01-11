use crate::prelude::Error;
use alloc::string::String;
use alloc::vec::Vec;
use pink_web3::{
    api::{Eth, Namespace},
    transports::{resolve_ready, PinkHttp},
    types::Address,
};

use pink_subrpc::{get_next_nonce, get_ss58addr_version, Ss58Codec};

#[derive(Clone, Debug, Default, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum ChainType {
    #[default]
    Evm,
    Sub,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Chain {
    pub id: u32,
    pub name: String,
    pub endpoint: String,
    pub chain_type: ChainType,
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

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use hex_literal::hex;
    use ink_lang as ink;

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
}
