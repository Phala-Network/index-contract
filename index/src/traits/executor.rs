#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use super::common::Address;
use super::common::Error;
use primitive_types::{H256, U256};

pub trait Executor {
    fn new(
        bridge_address: Address,
        abi_json: &[u8],
        rpc: &str,
    ) -> core::result::Result<Self, Error>
    where
        Self: Sized;
    fn transfer(
        &self,
        signer: [u8; 32], // FIXME
        token_rid: H256,
        amount: U256,
        recipient: Address,
    ) -> core::result::Result<(), Error>;
}

pub trait BridgeExecutor: Sized + 'static {
    fn new(assets: Vec<(Vec<u8>, [u8; 32])>) -> Self;

    fn transfer(&self, signer: [u8; 32], asset: Vec<u8>, recipient: Vec<u8>, amount: u128) -> core::result::Result<Self, Error>;
}

pub trait DexExecutor: Sized + 'static {
    fn new(router: Vec<u8>) -> Self;

    fn swap(&self, signer: [u8; 32], asset0: Vec<u8>, asset1: Vec<u8>, spend: u128, recipient: Vec<u8>) -> core::result::Result<Self, Error>;
}
