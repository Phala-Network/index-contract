use crate::prelude::Error;
use alloc::string::String;
use alloc::vec::Vec;
use pink_web3::{
    api::{Eth, Namespace},
    transports::{resolve_ready, PinkHttp},
    types::Address,
};

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
            ChainType::Sub => return Err(Error::Unimplemented),
        })
    }
}
