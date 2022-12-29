use scale::{Decode, Encode};

use crate::prelude::Error;

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
    fn get_nonce(&self, _account: Vec<u8>) -> core::result::Result<u64, Error> {
        Err(Error::Unimplemented)
    }
}
