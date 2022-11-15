use crate::traits::{
    common::{Address, Error},
    executor::DexExecutor,
};
use crate::transactors::ChainBridgeClient;
use ink_storage::Mapping;
use pink_web3::api::{Eth, Namespace};
use pink_web3::contract::Contract;
use pink_web3::keys::pink::KeyPair;
use pink_web3::transports::PinkHttp;
use primitive_types::{H256, U256};
use scale::Encode;
use xcm::v0::NetworkId;
use xcm::v1::{Junction, Junctions, MultiLocation};


pub struct UniswapV2Executor {
    // Router address
    router: Vec<u8>,
}
impl DexExecutor for UniswapV2Executor {
    fn new(router: Vec<u8>) -> Self {
        Self {
            router
        }
    }

    fn swap(&self, signer: [u8; 32], asset0: Vec<u8>, asset1: Vec<u8>, spend: u128, recipient: Vec<u8>) -> core::result::Result<Self, Error> {
        // Initialized ethereum contract entity

        // Execute RPC call router.populateTransaction.swapExactTokensForTokens

        Err(Error::Unimplemented)
    }
}