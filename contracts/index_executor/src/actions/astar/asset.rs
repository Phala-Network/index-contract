use crate::traits::AssetRegistry;
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use xcm::v3::{prelude::*, MultiLocation};

#[derive(Default)]
pub struct AstarAssets {
    // (chain, (asset_id, asset_location))
    id_to_location: Vec<(String, Vec<(u128, MultiLocation)>)>,
    // (chain, (asset_location, asset_id))
    location_to_id: Vec<(String, Vec<(MultiLocation, u128)>)>,
}

impl AstarAssets {
    pub fn new() -> Self {
        Self {
            id_to_location: vec![(
                "Astar".to_string(),
                vec![
                    // PHA
                    (
                        18446744073709551622_u128,
                        MultiLocation::new(1, X1(Parachain(crate::constants::PHALA_PARACHAIN_ID))),
                    ),
                    // GLMR
                    (
                        18446744073709551619_u128,
                        MultiLocation::new(
                            1,
                            X2(
                                Parachain(crate::constants::MOONBEAM_PARACHAIN_ID),
                                PalletInstance(10),
                            ),
                        ),
                    ),
                ],
            )],
            location_to_id: vec![(
                "Astar".to_string(),
                vec![
                    // PHA
                    (
                        MultiLocation::new(1, X1(Parachain(crate::constants::PHALA_PARACHAIN_ID))),
                        18446744073709551622_u128,
                    ),
                    // GLMR
                    (
                        MultiLocation::new(
                            1,
                            X2(
                                Parachain(crate::constants::MOONBEAM_PARACHAIN_ID),
                                PalletInstance(10),
                            ),
                        ),
                        18446744073709551619_u128,
                    ),
                ],
            )],
        }
    }

    pub fn get_location(&self, chain: &str, asset_id: u128) -> Option<MultiLocation> {
        match self.id_to_location.iter().position(|a| a.0 == chain) {
            Some(idx0) => self.id_to_location[idx0]
                .1
                .iter()
                .position(|a| a.0 == asset_id)
                .map(|idx1| self.id_to_location[idx0].1[idx1].1),
            _ => None,
        }
    }

    pub fn get_assetid(&self, chain: &str, location: &MultiLocation) -> Option<u128> {
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

impl AssetRegistry<u128> for AstarAssets {
    fn get_location(&self, chain: &str, asset_id: u128) -> Option<MultiLocation> {
        self.get_location(chain, asset_id)
    }
    fn get_assetid(&self, chain: &str, location: &MultiLocation) -> Option<u128> {
        self.get_assetid(chain, location)
    }
}
