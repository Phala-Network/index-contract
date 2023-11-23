use crate::actions::base::uniswapv2;

pub mod xtoken;

pub type MoonbeamStellaSwap = uniswapv2::UniswapV2;

use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::constants::*;
use alloc::{boxed::Box, string::String, vec, vec::Vec};

pub fn create_actions(chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    let stellaswap_router: [u8; 20] = hex_literal::hex!("70085a09D30D6f8C4ecF6eE10120d1847383BB57");
    let moonbeam_xtoken: [u8; 20] = hex_literal::hex!("0000000000000000000000000000000000000804");

    vec![
        (
            String::from("moonbeam_stellaswap"),
            Box::new(MoonbeamStellaSwap::new(
                &chain.endpoint,
                stellaswap_router.into(),
            )),
        ),
        (
            String::from("moonbeam_bridge_to_acala"),
            Box::new(xtoken::XTokenBridge::new(
                &chain.endpoint,
                moonbeam_xtoken.into(),
                ACALA_PARACHAIN_ID,
            )),
        ),
        (
            String::from("moonbeam_bridge_to_astar"),
            Box::new(xtoken::XTokenBridge::new(
                &chain.endpoint,
                moonbeam_xtoken.into(),
                ASTAR_PARACHAIN_ID,
            )),
        ),
        (
            String::from("moonbeam_bridge_to_phala"),
            Box::new(xtoken::XTokenBridge::new(
                &chain.endpoint,
                moonbeam_xtoken.into(),
                PHALA_PARACHAIN_ID,
            )),
        ),
    ]
}
