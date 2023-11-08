pub mod asset;
mod sub;
mod xtokens;

use crate::actions::base::{native_wrapper, uniswapv2};
use crate::actions::ActionExtraInfo;
use sp_runtime::Permill;

pub type AstarArthSwap = uniswapv2::UniswapV2;
pub type AstarNativeWrapper = native_wrapper::NativeWrapper;

use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::constants::PARACHAIN_BLOCK_TIME;
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
    ]
}

pub fn get_extra_info(chain: &str, action: &str) -> Option<ActionExtraInfo> {
    assert!(chain == "Astar" || chain == "AstarEvm");
    if action == "astar_evm_nativewrapper" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            const_proto_fee_in_usd: 0,
            percentage_proto_fee: Permill::zero(),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME,
        })
    } else if action == "astar_evm_arthswap" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            const_proto_fee_in_usd: 0,
            percentage_proto_fee: Permill::from_perthousand(3),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME,
        })
    } else if action == "astar_transactor" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            const_proto_fee_in_usd: 0,
            percentage_proto_fee: Permill::zero(),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME,
        })
    } else if action == "astar_bridge_to_astarevm" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            const_proto_fee_in_usd: 0,
            percentage_proto_fee: Permill::zero(),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME,
        })
    } else if action == "astar_bridge_to_phala" {
        Some(ActionExtraInfo {
            // 0.0005 USD
            const_proto_fee: 5,
            percentage_proto_fee: Permill::zero(),
            confirm_time: PARACHAIN_BLOCK_TIME,
        })
    } else {
        None
    }
}
