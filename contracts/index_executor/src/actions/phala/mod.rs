pub mod asset;
pub mod sygma;
pub mod xtransfer;

use crate::account::AccountType;
use crate::actions::base::sub_transactor;
use crate::actions::phala::xtransfer::XTransferDestChain;
use crate::actions::ActionExtraInfo;
use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::constants::{
    ACALA_PARACHAIN_ID, ASTAR_PARACHAIN_ID, ETHEREUM_BLOCK_TIME, MOONBEAM_PARACHAIN_ID,
    PARACHAIN_BLOCK_TIME, SYGMA_ETHEREUM_DOMAIN_ID,
};
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use sp_runtime::Permill;

pub fn create_actions(_chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    vec![
        (
            String::from("phala_bridge_to_acala"),
            Box::new(xtransfer::XTransferXcm::new(
                XTransferDestChain::ParaChain(ACALA_PARACHAIN_ID),
                AccountType::Account32,
            )),
        ),
        (
            String::from("phala_bridge_to_astar"),
            Box::new(xtransfer::XTransferXcm::new(
                XTransferDestChain::ParaChain(ASTAR_PARACHAIN_ID),
                AccountType::Account32,
            )),
        ),
        (
            String::from("phala_bridge_to_moonbeam"),
            Box::new(xtransfer::XTransferXcm::new(
                XTransferDestChain::ParaChain(MOONBEAM_PARACHAIN_ID),
                AccountType::Account20,
            )),
        ),
        (
            String::from("phala_bridge_to_polkadot"),
            Box::new(xtransfer::XTransferXcm::new(
                XTransferDestChain::RelayChain,
                AccountType::Account20,
            )),
        ),
        (
            String::from("phala_bridge_to_ethereum"),
            Box::new(sygma::XTransferSygma::new(SYGMA_ETHEREUM_DOMAIN_ID)),
        ),
        (
            String::from("khala_bridge_to_ethereum"),
            Box::new(sygma::XTransferSygma::new(SYGMA_ETHEREUM_DOMAIN_ID)),
        ),
        (
            String::from("phala_native_transactor"),
            Box::new(sub_transactor::Transactor::new(0x28, 0x07)),
        ),
        (
            String::from("khala_native_transactor"),
            Box::new(sub_transactor::Transactor::new(0x28, 0x07)),
        ),
    ]
}

#[allow(clippy::if_same_then_else)]
pub fn get_extra_info(chain: &str, action: &str) -> Option<ActionExtraInfo> {
    assert!(chain == "Phala" || chain == "Khala");
    if action == "phala_bridge_to_acala" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            // 0.0005 USD
            const_proto_fee_in_usd: 5,
            percentage_proto_fee: Permill::zero(),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME * 2,
        })
    } else if action == "phala_bridge_to_astar" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            // 0.0005 USD
            const_proto_fee_in_usd: 5,
            percentage_proto_fee: Permill::zero(),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME * 2,
        })
    } else if action == "phala_bridge_to_moonbeam" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            // 0.0005 USD
            const_proto_fee_in_usd: 5,
            percentage_proto_fee: Permill::zero(),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME * 2,
        })
    } else if action == "phala_bridge_to_polkadot" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            // 5 USD
            const_proto_fee_in_usd: 5,
            percentage_proto_fee: Permill::zero(),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME,
        })
    } else if action == "phala_bridge_to_ethereum" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            // 5 USD
            const_proto_fee_in_usd: 5000,
            percentage_proto_fee: Permill::zero(),
            // Sygma relayer wait 2 blocks to finialize and 1 block on Phala to confirm
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME * 2 + ETHEREUM_BLOCK_TIME,
        })
    } else if action == "phala_native_transactor" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            const_proto_fee_in_usd: 0,
            percentage_proto_fee: Permill::zero(),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME,
        })
    } else if action == "khala_bridge_to_ethereum" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            // 5 USD
            const_proto_fee_in_usd: 5000,
            percentage_proto_fee: Permill::zero(),
            // Sygma relayer wait 2 blocks to finialize and 1 block on Ethereum to confirm
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME * 2 + ETHEREUM_BLOCK_TIME,
        })
    } else if action == "khala_native_transactor" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            const_proto_fee_in_usd: 0,
            percentage_proto_fee: Permill::zero(),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME,
        })
    } else {
        None
    }
}
