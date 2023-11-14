pub mod asset;
mod sub;
mod xtokens;

use crate::actions::base::{evm_transactor, native_wrapper, uniswapv2};

pub type AstarArthSwap = uniswapv2::UniswapV2;
pub type AstarNativeWrapper = native_wrapper::NativeWrapper;

use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::constants::PHALA_PARACHAIN_ID;
use crate::utils::ToArray;
use alloc::{boxed::Box, string::String, vec, vec::Vec};

pub fn evm_create_actions(chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    let arthswap_pancake_router: [u8; 20] = hex::decode("E915D2393a08a00c5A463053edD31bAe2199b9e7")
        .unwrap()
        .to_array();
    let astar_evm_wastr: [u8; 20] = hex_literal::hex!("Aeaaf0e2c81Af264101B9129C00F4440cCF0F720");
    let astar_evm_astr: [u8; 20] = hex_literal::hex!("0000000000000000000000000000000000000000");

    vec![
        (
            String::from("astar_evm_nativewrapper"),
            Box::new(AstarNativeWrapper::new(
                &chain.endpoint,
                astar_evm_wastr.into(),
                astar_evm_astr.into(),
            )),
        ),
        (
            String::from("astar_evm_arthswap"),
            Box::new(AstarArthSwap::new(
                &chain.endpoint,
                arthswap_pancake_router.into(),
            )),
        ),
    ]
}

pub fn sub_create_actions(chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    vec![
        (
            String::from("astar_transactor"),
            Box::new(sub::AstarTransactor::new(chain.native_asset.clone())),
        ),
        (
            String::from("astar_bridge_to_astarevm"),
            Box::new(sub::AstarSubToEvmTransactor::new(
                chain.native_asset.clone(),
            )),
        ),
        (
            String::from("astar_bridge_to_phala"),
            Box::new(xtokens::AstarXtokens::new(PHALA_PARACHAIN_ID)),
        ),
        (
            String::from("astarevm_bridge_to_astar"),
            Box::new(evm_transactor::Transactor::new(
                &chain.endpoint,
                chain.native_asset.clone(),
            )),
        ),
        (
            String::from("astarevm_transactor"),
            Box::new(evm_transactor::Transactor::new(
                &chain.endpoint,
                chain.native_asset.clone(),
            )),
        ),
    ]
}
