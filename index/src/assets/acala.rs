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

// Copy from https://github.com/AcalaNetwork/Acala/blob/master/primitives/src/currency.rs ,
// with modification
//
//
// 0 - 127: Polkadot Ecosystem tokens
// 0 - 19: Acala & Polkadot native tokens
// 20 - 39: External tokens (e.g. bridged)
// 40 - 127: Polkadot parachain tokens
//
// 128 - 255: Kusama Ecosystem tokens
// 128 - 147: Karura & Kusama native tokens
// 148 - 167: External tokens (e.g. bridged)
// 168 - 255: Kusama parachain tokens
#[derive(Debug, Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[repr(u8)]
#[allow(clippy::upper_case_acronyms)]
#[allow(clippy::unnecessary_cast)]
pub enum TokenSymbol {
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

#[derive(Debug, Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CurrencyId {
    Token(TokenSymbol),
}

#[derive(Debug, Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum AggregatedSwapPath {
    Dex(Vec<CurrencyId>),
    Taiga(u32, u32, u32),
}

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

#[allow(dead_code)]
#[derive(Default)]
pub struct Currencyid2Location {
    // (chain, currency_id, asset_location)
    assets: Vec<(String, Vec<(CurrencyId, MultiLocation)>)>,
}
impl Currencyid2Location {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            assets: vec![
                (
                    "Karura".to_string(),
                    vec![
                        // KAR
                        (
                            CurrencyId::Token(TokenSymbol::KAR),
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
                            CurrencyId::Token(TokenSymbol::PHA),
                            MultiLocation::new(1, X1(Parachain(2004))),
                        ),
                    ],
                ),
                (
                    "Acala".to_string(),
                    vec![
                        // ACA
                        (
                            CurrencyId::Token(TokenSymbol::ACA),
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

    #[allow(dead_code)]
    pub fn get_location(&self, chain: String, currency_id: CurrencyId) -> Option<MultiLocation> {
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

#[derive(Default)]
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
                            CurrencyId::Token(TokenSymbol::KAR),
                        ),
                        // PHA
                        (
                            MultiLocation::new(1, X1(Parachain(2004))),
                            CurrencyId::Token(TokenSymbol::PHA),
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
                            CurrencyId::Token(TokenSymbol::ACA),
                        ),
                    ],
                ),
            ],
        }
    }

    pub fn get_currencyid(&self, chain: String, location: &MultiLocation) -> Option<CurrencyId> {
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
