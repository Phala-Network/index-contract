// TODO: Remove sp-runtime to decline size of wasm blob
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use scale::Decode;
use scale::Encode;
use sp_runtime::{traits::ConstU32, WeakBoundedVec};
use xcm::v1::{prelude::*, MultiLocation};

// Copy from https://github.com/AcalaNetwork/Acala/blob/master/primitives/src/currency.rs ,
// with modification
//
// 0 - 127: Polkadot Ecosystem tokens
// 0 - 19: Acala & Polkadot native tokens
// 20 - 39: External tokens (e.g. bridged)
// 40 - 127: Polkadot parachain tokens
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

const LC_KAR: MultiLocation = MultiLocation::new(
    1,
    X2(
        Parachain(2000),
        GeneralKey(WeakBoundedVec::<u8, ConstU32<32>>::force_from(
            vec![0x00, 0x80],
            None,
        )),
    ),
);
const LC_PHA: MultiLocation = MultiLocation::new(1, X1(Parachain(2004)));
const LC_ACA: MultiLocation = MultiLocation::new(
    1,
    X2(
        Parachain(2000),
        GeneralKey(WeakBoundedVec::<u8, ConstU32<32>>::force_from(
            vec![0x00, 0x00],
            None,
        )),
    ),
);
const LC_DOT: MultiLocation = MultiLocation::new(
    1,
    X2(
        Parachain(2000),
        GeneralKey(WeakBoundedVec::<u8, ConstU32<32>>::force_from(
            vec![0x00, 0x02],
            None,
        )),
    ),
);

pub type ForeignAssetId = u16;
const FA_GLMR: ForeignAssetId = 0;
const FA_PARA: ForeignAssetId = 1;
const FA_ASTR: ForeignAssetId = 2;
const FA_IBTC: ForeignAssetId = 3;
const FA_INTR: ForeignAssetId = 4;
const FA_WBTC: ForeignAssetId = 5;
const FA_WETH: ForeignAssetId = 6;
const FA_EQ: ForeignAssetId = 7;
const FA_EQD: ForeignAssetId = 8;
const FA_PHA: ForeignAssetId = 9;
const FA_UNQ: ForeignAssetId = 10;

#[derive(Debug, Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CurrencyId {
    Token(TokenSymbol),
    ForeignAsset(ForeignAssetId),
}

#[derive(Debug, Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum AggregatedSwapPath {
    Dex(Vec<CurrencyId>),
    Taiga(u32, u32, u32),
}

/// Acala has lots of token types which are handled in different way in case of transfer,
/// ACA(the utility token of Acala) goes the Balance::Transfer way
/// other tokens goes the Currencies::Transfer way,
/// the native tokens, dot is one of them, is wrapped as CurrencyId::TokenSymbol,
/// foreign tokens CurrencyId::ForeignAssetId
pub enum TokenType {
    Utility,
    Native,
    Foreign,
}

pub type TokenAttrs = (
    MultiLocation,
    TokenSymbol,
    TokenType,
    Option<ForeignAssetId>,
);
const TOKENS: Vec<TokenAttrs> = vec![
    (LC_ACA, TokenSymbol::ACA, TokenType::Utility, None),
    (LC_DOT, TokenSymbol::DOT, TokenType::Native, None),
    (LC_KAR, TokenSymbol::KAR, TokenType::Native, None),
    (LC_PHA, TokenSymbol::PHA, TokenType::Foreign, Some(FA_PHA)),
];

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
                        (CurrencyId::Token(TokenSymbol::KAR), LC_KAR),
                        (CurrencyId::Token(TokenSymbol::PHA), LC_PHA),
                    ],
                ),
                (
                    "Acala".to_string(),
                    vec![
                        (CurrencyId::Token(TokenSymbol::ACA), LC_ACA),
                        (CurrencyId::Token(TokenSymbol::DOT), LC_DOT),
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
pub struct AcalaAssetMap;
impl AcalaAssetMap {
    pub fn get_asset_attrs(
        location: &MultiLocation,
    ) -> Option<(TokenSymbol, TokenType, Option<ForeignAssetId>)> {
        let token = TOKENS.iter().find(|&&s| s.0 == location.clone());
        if let Some(token) = token {
            return Some((token.1, token.2, token.3));
        }
        None
    }
}
