pub mod asset;
pub mod dex;
pub mod transfer;

use crate::call::CallBuilder;
use crate::chain::Chain;
use alloc::{boxed::Box, string::String, vec, vec::Vec};

pub fn create_actions(_chain: &Chain) -> Vec<(String, Box<dyn CallBuilder>)> {
    vec![
        (String::from("acala_dex"), Box::new(dex::AcalaSwap::new())),
        (
            String::from("acala_transactor"),
            Box::new(transfer::AcalaTransactor::new()),
        ),
    ]
}
