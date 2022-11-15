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
