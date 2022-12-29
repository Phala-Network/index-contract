use alloc::{string::String, vec::Vec};
use ink_storage::traits::{PackedLayout, SpreadAllocate, SpreadLayout, StorageLayout};
use scale::{Decode, Encode};
use xcm::latest::{AssetId, MultiLocation};


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
