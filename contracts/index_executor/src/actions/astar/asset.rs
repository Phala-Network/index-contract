// TODO: Remove sp-runtime to decline size of wasm blob
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use xcm::v3::{prelude::*, MultiLocation};

#[allow(dead_code)]
#[derive(Default)]
pub struct Assetid2Location {
    // (chain, (asset_id, asset_location))
    assets: Vec<(String, Vec<(u128, MultiLocation)>)>,
}

impl Assetid2Location {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            assets: vec![(
                "Astar".to_string(),
                vec![
                    // PHA
                    (
                        18446744073709551622_u128,
                        MultiLocation::new(1, X1(Parachain(2035))),
                    ),
                ],
            )],
        }
    }

    #[allow(dead_code)]
    pub fn get_location(&self, chain: String, asset_id: u128) -> Option<MultiLocation> {
        match self.assets.iter().position(|a| a.0 == chain) {
            Some(idx0) => self.assets[idx0]
                .1
                .iter()
                .position(|a| a.0 == asset_id)
                .map(|idx1| self.assets[idx0].1[idx1].1),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Location2Assetid {
    // (chain, (asset_location, asset_id))
    assets: Vec<(String, Vec<(MultiLocation, u128)>)>,
}
impl Location2Assetid {
    pub fn new() -> Self {
        Self {
            assets: vec![(
                "Astar".to_string(),
                vec![
                    // PHA
                    (
                        MultiLocation::new(1, X1(Parachain(2035))),
                        18446744073709551622_u128,
                    ),
                ],
            )],
        }
    }

    pub fn get_assetid(&self, chain: String, location: &MultiLocation) -> Option<u128> {
        match self.assets.iter().position(|a| a.0 == chain) {
            Some(idx0) => self.assets[idx0]
                .1
                .iter()
                .position(|a| &a.0 == location)
                .map(|idx1| self.assets[idx0].1[idx1].1),
            _ => None,
        }
    }
}
