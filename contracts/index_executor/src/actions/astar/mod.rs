use crate::actions::base::uniswapv2;

pub type AstarArthSwap = uniswapv2::UniswapV2;

use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::utils::ToArray;
use alloc::{boxed::Box, string::String, vec, vec::Vec};

pub fn create_actions(chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    let arthswap_pancake_router: [u8; 20] = hex::decode("E915D2393a08a00c5A463053edD31bAe2199b9e7")
        .unwrap()
        .to_array();
    vec![(
        String::from("astar_arthswap"),
        Box::new(AstarArthSwap::new(
            &chain.endpoint,
            arthswap_pancake_router.into(),
        )),
    )]
}
