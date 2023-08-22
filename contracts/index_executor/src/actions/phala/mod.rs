pub mod asset;
pub mod sygma;
pub mod xtransfer;

use crate::account::AccountType;
use crate::call::CallBuilder;
use crate::chain::Chain;
use crate::constants::{
    ACALA_PARACHAIN_ID, ASTAR_PARACHAIN_ID, MOONBEAM_PARACHAIN_ID, SYGMA_ETHEREUM_DOMAIN_ID,
};
use alloc::{boxed::Box, string::String, vec, vec::Vec};

pub fn create_actions(_chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    vec![
        (
            String::from("phala_bridge_to_acala"),
            Box::new(xtransfer::XTransferXcm::new(
                ACALA_PARACHAIN_ID,
                AccountType::Account32,
            )),
        ),
        (
            String::from("phala_bridge_to_astar"),
            Box::new(xtransfer::XTransferXcm::new(
                ASTAR_PARACHAIN_ID,
                AccountType::Account32,
            )),
        ),
        (
            String::from("phala_bridge_to_moonbeam"),
            Box::new(xtransfer::XTransferXcm::new(
                MOONBEAM_PARACHAIN_ID,
                AccountType::Account20,
            )),
        ),
        (
            String::from("phala_bridge_to_ethereum"),
            Box::new(sygma::XTransferSygma::new(SYGMA_ETHEREUM_DOMAIN_ID)),
        ),
    ]
}
