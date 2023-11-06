mod xcm;

use crate::chain::Chain;
use crate::constants::{MOONBEAM_PARACHAIN_ID, PHALA_PARACHAIN_ID};
use crate::{account::AccountType, call::CallBuilder};
use alloc::{boxed::Box, string::String, vec, vec::Vec};

pub fn create_actions(_chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    vec![
        (
            String::from("polkadot_bridge_to_phala"),
            Box::new(xcm::PolkadotXcm::new(
                PHALA_PARACHAIN_ID,
                AccountType::Account20,
            )),
        ),
        (
            String::from("polkadot_bridge_to_moonbeam"),
            Box::new(xcm::PolkadotXcm::new(
                MOONBEAM_PARACHAIN_ID,
                AccountType::Account32,
            )),
        ),
    ]
}
