use crate::actions::acala::asset::AcalaAssets;
use crate::actions::astar::asset::AstarAssets;
use crate::actions::phala::asset::PhalaAssets;
use crate::traits::AssetRegistry;

use alloc::{string::String, vec, vec::Vec};
use scale::Encode;
use xcm::v3::MultiLocation;

/// Return empty if assetid not found
pub fn get_assetid_by_location(chain: &String, location: &MultiLocation) -> Vec<u8> {
    pink_extension::debug!("Lookup assert registry module for {:?}", &chain);
    if chain == "Acala" || chain == "Karura" {
        match AcalaAssets.get_assetid(chain, location) {
            Some(id) => id.encode().to_vec(),
            _ => vec![],
        }
    } else if chain == "Astar" {
        match AstarAssets::new().get_assetid(chain, location) {
            Some(id) => id.to_le_bytes().to_vec(),
            _ => vec![],
        }
    } else if chain == "Khala" || chain == "Phala" {
        match PhalaAssets::new().get_assetid(chain, location) {
            Some(id) => id.to_le_bytes().to_vec(),
            _ => vec![],
        }
    } else {
        vec![]
    }
}
