use crate::actions::base::{native_wrapper, uniswapv2, uniswapv3};
pub mod sygma;

use alloc::{boxed::Box, string::String, vec, vec::Vec};
pub type EthereumUniswapV2 = uniswapv2::UniswapV2;
pub type EthereumUniswapV3 = uniswapv3::UniswapV3;
pub type EthereumNativeWrapper = native_wrapper::NativeWrapper;

use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::utils::ToArray;
use core::str::FromStr;
use pink_web3::ethabi::Address;

pub fn create_actions(chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    let uniswapv2_router: [u8; 20] = hex::decode("7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
        .unwrap()
        .to_array();
    let uniswapv3_router: [u8; 20] = hex::decode("E592427A0AEce92De3Edee1F18E0157C05861564")
        .unwrap()
        .to_array();
    let ethereum_weth: [u8; 20] = hex_literal::hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    let ethereum_eth: [u8; 20] = hex_literal::hex!("0000000000000000000000000000000000000000");

    vec![
        (
            String::from("ethereum_nativewrapper"),
            Box::new(EthereumNativeWrapper::new(
                &chain.endpoint,
                ethereum_weth.into(),
                ethereum_eth.into(),
            )),
        ),
        (
            String::from("ethereum_uniswapv2"),
            Box::new(EthereumUniswapV2::new(
                &chain.endpoint,
                uniswapv2_router.into(),
            )),
        ),
        (
            String::from("ethereum_uniswapv3"),
            Box::new(EthereumUniswapV3::new(
                &chain.endpoint,
                uniswapv3_router.into(),
            )),
        ),
        (
            String::from("ethereum_sygmabridge_to_phala"),
            Box::new(sygma::EvmSygmaBridge::new(
                &chain.endpoint,
                Address::from_str("4D878E8Fb90178588Cda4cf1DCcdC9a6d2757089").unwrap(),
                Address::from_str("C832588193cd5ED2185daDA4A531e0B26eC5B830").unwrap(),
                Address::from_str("e43F8245249d7fAF46408723Ab36D071dD85D7BB").unwrap(),
                // 0.001 ETH
                100_000_000_000_000u128,
                1,
                3,
                None,
            )),
        ),
        (
            String::from("ethereum_sygmabridge_to_khala"),
            Box::new(sygma::EvmSygmaBridge::new(
                &chain.endpoint,
                Address::from_str("4D878E8Fb90178588Cda4cf1DCcdC9a6d2757089").unwrap(),
                Address::from_str("C832588193cd5ED2185daDA4A531e0B26eC5B830").unwrap(),
                Address::from_str("e43F8245249d7fAF46408723Ab36D071dD85D7BB").unwrap(),
                // 0.001 ETH
                100_000_000_000_000u128,
                1,
                2,
                None,
            )),
        ),
    ]
}
