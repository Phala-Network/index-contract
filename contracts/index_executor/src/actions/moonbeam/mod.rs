use crate::actions::base::{native_wrapper, uniswapv3};

pub mod xtoken;

pub type MoonbeamStellaSwap = uniswapv3::UniswapV3;
pub type MoonbeamNativeWrapper = native_wrapper::NativeWrapper;

use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::constants::*;
use crate::utils::ToArray;
use alloc::{boxed::Box, string::String, vec, vec::Vec};

pub fn create_actions(chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    let stellaswap_routerv3: [u8; 20] = hex::decode("e6d0ED3759709b743707DcfeCAe39BC180C981fe")
        .unwrap()
        .to_array();
    let moonbeam_xtoken: [u8; 20] = hex_literal::hex!("0000000000000000000000000000000000000804");
    let moonbeam_wglmr: [u8; 20] = hex_literal::hex!("Acc15dC74880C9944775448304B263D191c6077F");
    let moonbeam_glmr: [u8; 20] = hex_literal::hex!("0000000000000000000000000000000000000802");

    vec![
        (
            String::from("moonbeam_nativewrapper"),
            Box::new(MoonbeamNativeWrapper::new(
                &chain.endpoint,
                moonbeam_wglmr.into(),
                moonbeam_glmr.into(),
            )),
        ),
        (
            String::from("moonbeam_stellaswap"),
            Box::new(MoonbeamStellaSwap::new(
                &chain.endpoint,
                stellaswap_routerv3.into(),
            )),
        ),
        (
            String::from("moonbeam_bridge_to_acala"),
            Box::new(xtoken::XTokenBridge::new(
                &chain.endpoint,
                moonbeam_xtoken.into(),
                xtoken::XTokenDestChain::Parachain(ACALA_PARACHAIN_ID),
            )),
        ),
        (
            String::from("moonbeam_bridge_to_astar"),
            Box::new(xtoken::XTokenBridge::new(
                &chain.endpoint,
                moonbeam_xtoken.into(),
                xtoken::XTokenDestChain::Parachain(ASTAR_PARACHAIN_ID),
            )),
        ),
        (
            String::from("moonbeam_bridge_to_phala"),
            Box::new(xtoken::XTokenBridge::new(
                &chain.endpoint,
                moonbeam_xtoken.into(),
                xtoken::XTokenDestChain::Parachain(PHALA_PARACHAIN_ID),
            )),
        ),
        (
            String::from("moonbeam_bridge_to_polkadot"),
            Box::new(xtoken::XTokenBridge::new(
                &chain.endpoint,
                moonbeam_xtoken.into(),
                xtoken::XTokenDestChain::Relaychain,
            )),
        ),
    ]
}
