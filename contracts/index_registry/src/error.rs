use scale::{Decode, Encode};

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Error {
    BadAbi,
    BadOrigin,
    AssetAlreadyRegistered,
    AssetNotFound,
    BridgeAlreadyRegistered,
    BridgeNotFound,
    ChainAlreadyRegistered,
    ChainNotFound,
    DexAlreadyRegistered,
    DexNotFound,
    ExtractLocationFailed,
    ConstructContractFailed,
    Unimplemented,
}
