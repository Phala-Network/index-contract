extern crate alloc;
use core::ops::Add;

use crate::traits::{common::Error, executor::DexExecutor};
use crate::transactors::UniswapV2Client;
use alloc::vec::Vec;
use pink_web3::types::Address;

use pink_web3::{
    api::{Eth, Namespace},
    contract::Contract,
    keys::pink::KeyPair,
    transports::PinkHttp,
};
use primitive_types::U256;

#[allow(dead_code)]
#[derive(Clone)]
pub struct MoonbeamDexExecutor {
    dex_contract: UniswapV2Client,
}

#[allow(dead_code)]
impl MoonbeamDexExecutor {
    pub fn new(rpc: &str, router: Address) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let dex_contract = UniswapV2Client {
            contract: Contract::from_json(
                eth,
                router,
                include_bytes!("../../abis/UniswapV2Router02.json"),
            )
            .expect("Bad abi data"),
        };

        Self { dex_contract }
    }
}

#[allow(dead_code)]
impl DexExecutor for MoonbeamDexExecutor {
    fn swap(
        &self,
        signer: [u8; 32],
        asset0: Vec<u8>,
        asset1: Vec<u8>,
        spend: u128,
        recipient: Vec<u8>,
    ) -> core::result::Result<(), Error> {
        let signer = KeyPair::from(signer);
        let asset0 = Address::from_slice(&asset0);
        let asset1 = Address::from_slice(&asset1);
        let to = Address::from_slice(&recipient);
        let path = [asset0, asset1];
        let amount_out = U256::from(0);
        let amount_in = U256::from(spend);
        let time = pink_extension::ext().untrusted_millis_since_unix_epoch() / 1000;
        let deadline = U256::from(time + 60 * 30);
        _ = self
            .dex_contract
            .swap(signer, amount_in, amount_out, path, to, deadline);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch() {
        pink_extension_runtime::mock_ext::mock_all_ext();
        let epoch = pink_extension::ext().untrusted_millis_since_unix_epoch();
        dbg!(epoch);
        assert!(epoch > 1673262288822 && epoch < 1773262288822);
        let shrunk = epoch / 1000;
        assert!(shrunk > 1673261721);
    }
}
