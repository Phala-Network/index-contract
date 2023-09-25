use crate::actions::base::uniswapv2;
use alloc::{boxed::Box, string::String, vec, vec::Vec};
pub type EthereumUniswapV2 = uniswapv2::UniswapV2;

use crate::call::CallBuilder;
use crate::chain::Chain;

pub fn create_actions(chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    let uniswapv2_router: [u8; 20] = hex_literal::hex!("7a250d5630B4cF539739dF2C5dAcb4c659F2488D");

    vec![(
        String::from("ethereum_uniswapv2"),
        Box::new(EthereumUniswapV2::new(
            &chain.endpoint,
            uniswapv2_router.into(),
        )),
    )]
}
