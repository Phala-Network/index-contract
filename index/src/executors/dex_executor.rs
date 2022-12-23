
extern crate alloc;
use alloc::vec::Vec;
use crate::traits::{common::Error, executor::DexExecutor};
use pink_web3::types::Address;

#[allow(dead_code)]
pub struct UniswapV2Executor {
    // Router address
    router: Address,
}

#[allow(dead_code)]
impl UniswapV2Executor {
    fn new(router: Address) -> Self {
        Self { router }
    }
}

#[allow(dead_code)]
impl DexExecutor for UniswapV2Executor {
    fn swap(
        &self,
        _signer: [u8; 32],
        _asset0: Vec<u8>,
        _asset1: Vec<u8>,
        _spend: u128,
        _recipient: Vec<u8>,
    ) -> core::result::Result<(), Error> {
        // Create UniswapV2 router contract entity

        // Execute RPC call router.populateTransaction.swapExactTokensForTokens

        Err(Error::Unimplemented)
    }
}
