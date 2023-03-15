extern crate alloc;

use crate::traits::{common::Error, executor::DexExecutor};
use crate::transactors::UniswapV2Client;
use alloc::vec;
use alloc::vec::Vec;
use pink_subrpc::ExtraParam;
use pink_web3::types::Address;

use pink_web3::{
    api::{Eth, Namespace},
    contract::Contract,
    keys::pink::KeyPair,
    transports::PinkHttp,
    types::U256,
};

#[allow(dead_code)]
#[derive(Clone)]
pub struct UniswapBasedExecutor {
    dex_contract: UniswapV2Client,
}

pub type ArthDexExecutor = UniswapBasedExecutor;
pub type MoonbeamDexExecutor = UniswapBasedExecutor;

#[allow(dead_code)]
impl UniswapBasedExecutor {
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
impl DexExecutor for UniswapBasedExecutor {
    fn swap(
        &self,
        signer: [u8; 32],
        asset0: Vec<u8>,
        asset1: Vec<u8>,
        spend: u128,
        recipient: Vec<u8>,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let signer = KeyPair::from(signer);
        let asset0 = Address::from_slice(&asset0);
        let asset1 = Address::from_slice(&asset1);
        let to = Address::from_slice(&recipient);
        let path = vec![asset0, asset1];
        let amount_out = U256::from(1);
        let amount_in = U256::from(spend);
        let time = pink_extension::ext().untrusted_millis_since_unix_epoch() / 1000;
        let deadline = U256::from(time + 60 * 30);
        let tx_id = self.dex_contract.swap(
            signer,
            amount_in,
            amount_out,
            path,
            to,
            deadline,
            extra.nonce,
        )?;
        Ok(tx_id.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::ToArray;
    use core::str::FromStr;
    use pink_subrpc::ExtraParam;
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
    fn arth_swap_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        // interacted with PancakeRouter
        // https://blockscout.com/astar/address/0xE915D2393a08a00c5A463053edD31bAe2199b9e7
        let executor = UniswapBasedExecutor::new(
            "https://astar.public.blastapi.io",
            H160::from_str("0xE915D2393a08a00c5A463053edD31bAe2199b9e7").unwrap(),
        );
        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();
        let recipient = hex::decode("49AFC6BBC7cCC19b25b3fe623d37aea2ab1Ee4eC").unwrap();

        let pha = hex::decode("ffffffff00000000000000010000000000000006").unwrap();
        let wastr = hex::decode("aeaaf0e2c81af264101b9129c00f4440ccf0f720").unwrap();

        // 0.1 wastr
        let spend: u128 = 100_000_000_000_000_000;
        let tx_id = executor
            .swap(signer, wastr, pha, spend, recipient, ExtraParam::default())
            .unwrap();
        dbg!(hex::encode(tx_id));
        // tx:
        //  - https://blockscout.com/astar/tx/0xb7fe0abc9c043c97296c094429b5b8e3bfcf9c330aad0d5f3cf37108881d3381
        //  - https://astar.subscan.io/tx/0x04ee52e3aeca297e5387bb0d57dab6b609ce110b5decb06b72515204446b6f70
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
            .swap(
                signer,
                wglmr,
                xc_dot,
                spend,
                recipient,
                ExtraParam::default(),
            )
            .unwrap();
    }
}