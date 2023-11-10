pub mod asset;
pub mod sygma;
pub mod xtransfer;

use crate::account::AccountType;
use crate::actions::base::sub_transactor;
use crate::actions::phala::xtransfer::XTransferDestChain;
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
