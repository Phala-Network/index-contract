use crate::actions::base::{native_wrapper, uniswapv2, uniswapv3};
use alloc::{boxed::Box, string::String, vec, vec::Vec};
pub type EthereumUniswapV2 = uniswapv2::UniswapV2;
pub type EthereumUniswapV3 = uniswapv3::UniswapV3;
pub type EthereumNativeWrapper = native_wrapper::NativeWrapper;

use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::utils::ToArray;

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
    ]
}
