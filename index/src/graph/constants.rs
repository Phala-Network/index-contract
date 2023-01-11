use sp_runtime::{traits::ConstU32, WeakBoundedVec};
use xcm::v1::{prelude::*, MultiLocation};

pub mod assets {
    use super::*;

    pub struct Assetid2Location {
        assets: Vec<(String, Vec<(u32, MultiLocation)>)>,
    }
    impl Assetid2Location {
        pub fn new() -> Self {
            Self {
                assets: vec![(
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
                )],
            }
        }

        pub fn get_location(&self, chain: String, asset_id: u32) -> Option<MultiLocation> {
            None
        }
    }

    pub struct Location2Assetid {
        assets: Vec<(String, Vec<(MultiLocation, u32)>)>,
    }
    impl Location2Assetid {
        pub fn new() -> Self {
            Self {
                assets: vec![(
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
                )],
            }
        }

        pub fn get_assetid(&self, chain: String, location: &MultiLocation) -> Option<u32> {
            None
        }
    }

    pub struct Currencyid2Location {
        assets: Vec<(String, Vec<(u32, MultiLocation)>)>,
    }
    impl Currencyid2Location {
        pub fn new() -> Self {
            Self {
                assets: vec![
                    (
                        "Karura".to_string(),
                        vec![
                            // KAR
                            (
                                0x0080,
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
                            // PHA
                            (0x00aa, MultiLocation::new(1, X1(Parachain(2004)))),
                        ],
                    ),
                    (
                        "Acala".to_string(),
                        vec![
                            // ACA
                            (
                                0x0000,
                                MultiLocation::new(
                                    1,
                                    X2(
                                        Parachain(2000),
                                        GeneralKey(WeakBoundedVec::<u8, ConstU32<32>>::force_from(
                                            vec![0x00, 0x00],
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

        pub fn get_location(&self, chain: String, currency_id: u32) -> Option<MultiLocation> {
            None
        }
    }

    pub struct Location2Currencyid {
        assets: Vec<(String, Vec<(MultiLocation, u32)>)>,
    }
    impl Location2Currencyid {
        pub fn new() -> Self {
            Self {
                assets: vec![
                    (
                        "Karura".to_string(),
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
                                0x0080,
                            ),
                            // PHA
                            (MultiLocation::new(1, X1(Parachain(2004))), 0x00aa),
                        ],
                    ),
                    (
                        "Acala".to_string(),
                        vec![
                            // ACA
                            (
                                MultiLocation::new(
                                    1,
                                    X2(
                                        Parachain(2000),
                                        GeneralKey(WeakBoundedVec::<u8, ConstU32<32>>::force_from(
                                            vec![0x00, 0x00],
                                            None,
                                        )),
                                    ),
                                ),
                                0x0000,
                            ),
                        ],
                    ),
                ],
            }
        }

        pub fn get_currencyid(&self, chain: String, location: &MultiLocation) -> Option<u32> {
            None
        }
    }
}
