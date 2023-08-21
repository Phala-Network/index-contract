use crate::utils::slice_to_generalkey;
use alloc::{vec, vec::Vec};

use scale::Decode;
use scale::Encode;
use xcm::v3::{prelude::*, MultiLocation};

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

pub type ForeignAssetId = u16;
#[allow(dead_code)]
const FA_GLMR: ForeignAssetId = 0;
#[allow(dead_code)]
const FA_PARA: ForeignAssetId = 1;
#[allow(dead_code)]
const FA_ASTR: ForeignAssetId = 2;
#[allow(dead_code)]
const FA_IBTC: ForeignAssetId = 3;
#[allow(dead_code)]
const FA_INTR: ForeignAssetId = 4;
#[allow(dead_code)]
const FA_WBTC: ForeignAssetId = 5;
#[allow(dead_code)]
const FA_WETH: ForeignAssetId = 6;
#[allow(dead_code)]
const FA_EQ: ForeignAssetId = 7;
#[allow(dead_code)]
const FA_EQD: ForeignAssetId = 8;
#[allow(dead_code)]
const FA_PHA: ForeignAssetId = 9;
#[allow(dead_code)]
const FA_UNQ: ForeignAssetId = 10;

#[derive(Debug, Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CurrencyId {
    Token(TokenSymbol),
    // placeholds to achieve correct encoding
    DexShare,
    Erc20,
    StableAssetPoolToken,
    LiquidCrowdload,
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
#[derive(Debug, Clone, Copy)]
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

#[derive(Default)]
pub struct AcalaAssetMap;

impl AcalaAssetMap {
    pub fn get_map() -> Vec<TokenAttrs> {
        let lc_kar: MultiLocation = MultiLocation::new(
            1,
            X2(Parachain(2000), slice_to_generalkey(&vec![0x00, 0x80])),
        );
        let lc_pha: MultiLocation = MultiLocation::new(1, X1(Parachain(2004)));
        let lc_aca: MultiLocation = MultiLocation::new(
            1,
            X2(Parachain(2000), slice_to_generalkey(&vec![0x00, 0x00])),
        );
        let lc_dot: MultiLocation = MultiLocation::new(
            1,
            X2(Parachain(2000), slice_to_generalkey(&vec![0x00, 0x02])),
        );
        vec![
            (lc_aca, TokenSymbol::ACA, TokenType::Utility, None),
            (lc_dot, TokenSymbol::DOT, TokenType::Native, None),
            (lc_kar, TokenSymbol::KAR, TokenType::Native, None),
            (lc_pha, TokenSymbol::PHA, TokenType::Foreign, Some(FA_PHA)),
        ]
    }

    pub fn get_asset_attrs(
        location: &MultiLocation,
    ) -> Option<(TokenSymbol, TokenType, Option<ForeignAssetId>)> {
        let tokens = AcalaAssetMap::get_map();
        let token = tokens.iter().find(|s| s.0 == location.clone());
        if let Some(token) = token {
            return Some((token.1, token.2, token.3));
        }
        None
    }

    pub fn get_currency_id(location: &MultiLocation) -> Option<CurrencyId> {
        if let Some(attrs) = AcalaAssetMap::get_asset_attrs(location) {
            return Some(CurrencyId::Token(attrs.0));
        }
        None
    }
}
