// TODO: Remove sp-runtime to decline size of wasm blob
use sp_runtime::{traits::ConstU32, WeakBoundedVec};
use xcm::v1::{prelude::*, MultiLocation};
use alloc::{vec, vec::Vec, string::{ToString, String}};

// Chainbridge chain ID
// pub(crate) const CHAINBRIDGE_ID_ETHEREUM: u8 = 0;
// pub(crate) const CHAINBRIDGE_ID_MOONRIVER: u8 = 2;
pub(crate) const CHAINBRIDGE_ID_PHALA: u8 = 3;
pub(crate) const ACALA_PARACHAIN_ID: u32 = 2000;
#[allow(dead_code)]
pub(crate) const PHALA_PARACHAIN_ID: u32 = 2035;

pub mod assets {
    use super::*;

    // Copy from https://github.com/AcalaNetwork/Acala/blob/master/primitives/src/currency.rs ,
    #[derive(Debug, scale::Encode, scale::Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
    pub enum CurrencyTokenSymbol {
        // 0 - 19: Acala & Polkadot native tokens
        ACA = 0,
        AUSD = 1,
        DOT = 2,
        LDOT = 3,
        TAP = 4,
        // 20 - 39: External tokens (e.g. bridged)
        RENBTC = 20,
        CASH = 21,
        // 40 - 127: Polkadot parachain tokens

        // 128 - 147: Karura & Kusama native tokens
        KAR = 128,
        KUSD = 129,
        KSM = 130,
        LKSM = 131,
        TAI = 132,
        // 148 - 167: External tokens (e.g. bridged)
        // 149: Reserved for renBTC
        // 150: Reserved for CASH
        // 168 - 255: Kusama parachain tokens
        BNC = 168,
        VSKSM = 169,
        PHA = 170,
        KINT = 171,
        KBTC = 172,
    }

    #[derive(Debug, scale::Encode, scale::Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
    pub enum CurrencyId {
        Token(CurrencyTokenSymbol),
    }

    pub struct Assetid2Location {
        // (chain, (asset_id, asset_location))
        assets: Vec<(String, Vec<(u32, MultiLocation)>)>,
    }
    impl Assetid2Location {
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

    pub struct Currencyid2Location {
        // (chain, currency_id, asset_location)
        assets: Vec<(String, Vec<(CurrencyId, MultiLocation)>)>,
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
                                CurrencyId::Token(CurrencyTokenSymbol::KAR),
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
                            (
                                CurrencyId::Token(CurrencyTokenSymbol::PHA),
                                MultiLocation::new(1, X1(Parachain(2004))),
                            ),
                        ],
                    ),
                    (
                        "Acala".to_string(),
                        vec![
                            // ACA
                            (
                                CurrencyId::Token(CurrencyTokenSymbol::ACA),
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

        pub fn get_location(
            &self,
            chain: String,
            currency_id: CurrencyId,
        ) -> Option<MultiLocation> {
            match self.assets.iter().position(|a| a.0 == chain) {
                Some(idx0) => self.assets[idx0]
                    .1
                    .iter()
                    .position(|a| a.0 == currency_id)
                    .map(|idx1| self.assets[idx0].1[idx1].1.clone()),
                _ => None,
            }
        }
    }

    pub struct Location2Currencyid {
        // (chain, (asset_location, currency_id))
        assets: Vec<(String, Vec<(MultiLocation, CurrencyId)>)>,
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
                                CurrencyId::Token(CurrencyTokenSymbol::KAR),
                            ),
                            // PHA
                            (
                                MultiLocation::new(1, X1(Parachain(2004))),
                                CurrencyId::Token(CurrencyTokenSymbol::PHA),
                            ),
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
                                CurrencyId::Token(CurrencyTokenSymbol::ACA),
                            ),
                        ],
                    ),
                ],
            }
        }

        pub fn get_currencyid(
            &self,
            chain: String,
            location: &MultiLocation,
        ) -> Option<CurrencyId> {
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
}
