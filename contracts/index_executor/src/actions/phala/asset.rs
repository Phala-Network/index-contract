use crate::traits::AssetRegistry;
use crate::utils::slice_to_generalkey;
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use xcm::v3::{prelude::*, MultiLocation};

#[derive(Default)]
pub struct PhalaAssets {
    // (chain, (asset_id, asset_location))
    id_to_location: Vec<(String, Vec<(u32, MultiLocation)>)>,
    // (chain, (asset_location, asset_id))
    location_to_id: Vec<(String, Vec<(MultiLocation, u32)>)>,
}

impl PhalaAssets {
    pub fn new() -> Self {
        Self {
            id_to_location: vec![
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
            location_to_id: vec![
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

    pub fn get_location(&self, chain: &str, asset_id: u32) -> Option<MultiLocation> {
        match self.id_to_location.iter().position(|a| a.0 == chain) {
            Some(idx0) => self.id_to_location[idx0]
                .1
                .iter()
                .position(|a| a.0 == asset_id)
                .map(|idx1| self.id_to_location[idx0].1[idx1].1),
            _ => None,
        }
    }

    pub fn get_assetid(&self, chain: &str, location: &MultiLocation) -> Option<u32> {
        match self.location_to_id.iter().position(|a| a.0 == chain) {
            Some(idx0) => self.location_to_id[idx0]
                .1
                .iter()
                .position(|a| &a.0 == location)
                .map(|idx1| self.location_to_id[idx0].1[idx1].1),
            _ => None,
        }
    }
}

impl AssetRegistry<u32> for PhalaAssets {
    fn get_location(&self, chain: &str, asset_id: u32) -> Option<MultiLocation> {
        self.get_location(chain, asset_id)
    }
    fn get_assetid(&self, chain: &str, location: &MultiLocation) -> Option<u32> {
        self.get_assetid(chain, location)
    }
}
