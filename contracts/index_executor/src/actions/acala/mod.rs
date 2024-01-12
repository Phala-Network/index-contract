pub mod asset;
pub mod dex;
pub mod transfer;

use crate::actions::ActionExtraInfo;
use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::constants::PARACHAIN_BLOCK_TIME;
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use sp_runtime::Permill;

pub fn create_actions(_chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    vec![
        (String::from("acala_dex"), Box::new(dex::AcalaSwap::new())),
        (
            String::from("acala_transactor"),
            Box::new(transfer::AcalaTransactor::new()),
        ),
    ]
}

pub fn get_extra_info(chain: &str, action: &str) -> Option<ActionExtraInfo> {
    assert!(chain == "Acala");
    if action == "acala_dex" {
        Some(ActionExtraInfo {
            extra_proto_fee_in_usd: 0,
            const_proto_fee_in_usd: 0,
            percentage_proto_fee: Permill::from_perthousand(3),
            confirm_time_in_sec: PARACHAIN_BLOCK_TIME,
        })
    } else if action == "acala_transactor" {
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
