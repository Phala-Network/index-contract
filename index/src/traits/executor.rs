extern crate alloc;
use super::common::Error;
use alloc::vec::Vec;
use dyn_clone::DynClone;
use pink_subrpc::ExtraParam;

pub trait BridgeExecutor: DynClone {
    fn transfer(
        &self,
        signer: [u8; 32],
        asset: Vec<u8>,
        recipient: Vec<u8>,
        amount: u128,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error>;
}

pub trait DexExecutor: DynClone {
    fn swap(
        &self,
        signer: [u8; 32],
        asset0: Vec<u8>,
        asset1: Vec<u8>,
        spend: u128,
        recipient: Vec<u8>,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error>;
}
