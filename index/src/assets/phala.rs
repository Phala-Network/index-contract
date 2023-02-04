// TODO: Remove sp-runtime to decline size of wasm blob
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use sp_runtime::{traits::ConstU32, WeakBoundedVec};
use xcm::v1::{prelude::*, MultiLocation};

use scale::Decode;
use scale::Encode;

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
                                X2(
                                    Parachain(2000),
                                    GeneralKey(WeakBoundedVec::<u8, ConstU32<32>>::force_from(
                                        vec![0x00, 0x80],
                                        None,
                                    )),
                                ),
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
                .map(|idx1| self.assets[idx0].1[idx1].1.clone()),
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
                                X2(
                                    Parachain(2000),
                                    GeneralKey(WeakBoundedVec::<u8, ConstU32<32>>::force_from(
                                        vec![0x00, 0x80],
                                        None,
                                    )),
                                ),
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
