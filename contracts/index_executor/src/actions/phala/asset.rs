use crate::utils::slice_to_generalkey;
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
    assets: Vec<(String, Vec<(u32, MultiLocation)>)>,
}

impl Assetid2Location {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            assets: vec![
                (
                    "Phala".to_string(),
                    vec![
                        // DOT
                        (0, MultiLocation::new(1, Here)),
                        // GLMR
                        (
                            1,
                            MultiLocation::new(1, X2(Parachain(2004), PalletInstance(10))),
                        ),
                    ],
                ),
                (
                    "Khala".to_string(),
                    vec![
                        // KAR
                        (
                            1,
                            MultiLocation::new(
                                1,
                                X2(Parachain(2000), slice_to_generalkey(&[0x00, 0x80])),
                            ),
                        ),
                    ],
                ),
            ],
        }
    }

    #[allow(dead_code)]
    pub fn get_location(&self, chain: String, asset_id: u32) -> Option<MultiLocation> {
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
    assets: Vec<(String, Vec<(MultiLocation, u32)>)>,
}
impl Location2Assetid {
    pub fn new() -> Self {
        Self {
            assets: vec![
                (
                    "Phala".to_string(),
                    vec![
                        // DOT
                        (MultiLocation::new(1, Here), 0),
                        // GLMR
                        (
                            MultiLocation::new(1, X2(Parachain(2004), PalletInstance(10))),
                            1,
                        ),
                    ],
                ),
                (
                    "Khala".to_string(),
                    vec![
                        // KAR
                        (
                            MultiLocation::new(
                                1,
                                X2(Parachain(2000), slice_to_generalkey(&[0x00, 0x80])),
                            ),
                            1,
                        ),
                    ],
                ),
            ],
        }
    }

    pub fn get_assetid(&self, chain: String, location: &MultiLocation) -> Option<u32> {
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
