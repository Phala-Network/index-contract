extern crate alloc;

use crate::traits::{common::Error, executor::DexExecutor};
use crate::transactors::UniswapV2Client;
use alloc::vec;
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
        //let wglmr = H160::from_str("0xacc15dc74880c9944775448304b263d191c6077f").unwrap();
        //let path = vec![asset0, wglmr, asset1];
        let path = vec![asset0, asset1];
        let amount_out = U256::from(1);
        let amount_in = U256::from(spend);
        let time = pink_extension::ext().untrusted_millis_since_unix_epoch() / 1000;
        let deadline = U256::from(time + 60 * 30);
        _ = self
            .dex_contract
            .swap(signer, amount_in, amount_out, path, to, deadline)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::ToArray;
    use core::str::FromStr;
    use primitive_types::H160;

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

    #[test]
    #[ignore]
    fn stella_swap_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let executor = MoonbeamDexExecutor::new(
            "https://moonbeam.public.blastapi.io",
            // https://docs.stellaswap.com/developers/smart-contracts#router-smart-contract-details
            H160::from_str("0x70085a09D30D6f8C4ecF6eE10120d1847383BB57").unwrap(),
        );
        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();
        let recipient = hex::decode("Ff2109923cE53C04f88aF0deBB411A8b51654f3B").unwrap();

        //let usdc = hex::decode("931715FEE2d06333043d11F658C8CE934aC61D0c").unwrap();
        let xc_dot = hex::decode("FfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080").unwrap();
        let wglmr = hex::decode("Acc15dC74880C9944775448304B263D191c6077F").unwrap();

        // 0.001 wglmr
        let spend: u128 = 1_000_000_000_000_000;
        // https://moonbeam.moonscan.io/tx/0x727b7e9b4d889762050c310942ea1818f8c32fd483e973e42c77ce034e37a5c6
        // https://moonbeam.moonscan.io/tx/0x742504fe490ecb8ab968ecdbdde2aa774d4eca43c0eb73ad539e9bb974011722
        executor
            .swap(signer, wglmr, xc_dot, spend, recipient)
            .unwrap();
    }
}
