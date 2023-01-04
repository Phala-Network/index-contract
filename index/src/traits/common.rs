use primitive_types::{H160, H256, U256};
use scale::{Decode, Encode};

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Error {
    BadAbi,
    BadOrigin,
    ExtractLocationFailed,
    InvalidAddress,
    ConstructContractFailed,
    FetchDataFailed,
    Unimplemented,
    InvalidMultilocation,
    InvalidAmount,
    SubRPCRequestFailed,
    InvalidBody,
    InvalidSignature,
    Ss58,
    FailedToGetGas,
    FailedToSubmitTransaction,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Address {
    EthAddr(H160),
    SubAddr(H256),
}

pub enum Amount {
    U256(U256),
    U128(u128),
}
