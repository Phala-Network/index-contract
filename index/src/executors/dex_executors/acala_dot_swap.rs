use pink_subrpc::{create_transaction, send_transaction, ExtraParam};

use crate::prelude::DexExecutor;
use crate::traits::common::Error;
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use scale::Decode;
use scale::{Compact, Encode};

#[derive(Clone)]
pub struct AcalaDotSwapExecutor {
    rpc: String,
}

#[allow(dead_code)]
impl AcalaDotSwapExecutor {
    pub fn new(rpc: &str) -> Self
    where
        Self: Sized,
    {
        Self {
            rpc: rpc.to_string(),
        }
    }
}

impl AcalaDotSwapExecutor {
    fn aggregated_swap(
        &self,
        signer: [u8; 32],
        path: Vec<u8>,
        spend: u128,
    ) -> core::result::Result<Vec<u8>, Error> {
        let amount_out = Compact(1_u8);
        let amount_in = Compact(spend);

        let mut path = path.as_ref();
        let path =
            Vec::<AggregatedSwapPath>::decode(&mut path).map_err(|_| Error::FailedToScaleDecode)?;
        let signed_tx = create_transaction(
            &signer,
            "acala",
            &self.rpc,
            // the call index of acala dex module
            //0x5b_u8,
            // the call index of acala aggregateddex module
            0x5d_u8,
            0x0u8,
            (path, amount_in, amount_out),
            ExtraParam::default(),
        )
        .map_err(|_| Error::InvalidSignature)?;
        let tx_id =
            send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)?;
        Ok(tx_id)
    }
}

impl DexExecutor for AcalaDotSwapExecutor {
    fn swap(
        &self,
        signer: [u8; 32],
        _asset0: Vec<u8>,
        _asset1: Vec<u8>,
        spend: u128,
        _recipient: Vec<u8>,
    ) -> core::result::Result<(), Error> {
        let token_ldot = CurrencyId::Token(TokenSymbol::LDOT);
        let token_ausd = CurrencyId::Token(TokenSymbol::AUSD);

        let taiga_path = AggregatedSwapPath::Taiga(0, 0, 1);
        let dex_path = AggregatedSwapPath::Dex(vec![token_ldot, token_ausd]);

        let path = vec![taiga_path, dex_path];
        let path = path.encode();

        _ = self.aggregated_swap(signer, path, spend)?;

        Ok(())
    }
}

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
