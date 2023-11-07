mod xcm_v2;
mod xcm_v3;

use crate::chain::Chain;
use crate::constants::{ASTAR_PARACHAIN_ID, MOONBEAM_PARACHAIN_ID, PHALA_PARACHAIN_ID};
use crate::{account::AccountType, call::CallBuilder};
use alloc::{boxed::Box, string::String, vec, vec::Vec};

pub fn create_actions(_chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    vec![
        (
            String::from("polkadot_bridge_to_phala"),
            Box::new(xcm_v3::PolkadotXcm::new(
                PHALA_PARACHAIN_ID,
                AccountType::Account32,
                false,
            )),
        ),
        (
            String::from("polkadot_bridge_to_moonbeam"),
            Box::new(xcm_v2::PolkadotXcm::new(
                MOONBEAM_PARACHAIN_ID,
                AccountType::Account20,
            )),
        ),
        (
            String::from("polkadot_bridge_to_astar_evm"),
            Box::new(xcm_v3::PolkadotXcm::new(
                ASTAR_PARACHAIN_ID,
                AccountType::Account32,
                true,
            )),
        ),
        (
            String::from("polkadot_bridge_to_astar"),
            Box::new(xcm_v3::PolkadotXcm::new(
                ASTAR_PARACHAIN_ID,
                AccountType::Account32,
                false,
            )),
        ),
    ]
}
