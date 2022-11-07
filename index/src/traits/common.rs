use primitive_types::{H160, H256};
use scale::{Decode, Encode};

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Error {
    BadAbi,
    BadOrigin,
    AssetAlreadyRegistered,
    AssetNotFound,
    ChainAlreadyRegistered,
    ChainNotFound,
    ExtractLocationFailed,
    InvalidAddress,
    ConstructContractFailed,
    FetchDataFailed,
    Unimplemented,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Address {
    EthAddr(H160),
    SubAddr(H256),
}
