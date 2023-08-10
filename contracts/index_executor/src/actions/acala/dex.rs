use super::asset::{AcalaAssetMap, AggregatedSwapPath, CurrencyId, TokenSymbol};
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use pink_extension::ResultExt;
use scale::{Compact, Decode, Encode};
use xcm::v1::prelude::*;

use crate::call::{Call, CallBuilder, CallParams, SubCall, SubUnsignedExtrinsic};
use crate::step::Step;

#[derive(Clone)]
pub struct AcalaSwap {
    rpc: String,
}

#[allow(dead_code)]
impl AcalaSwap {
    pub fn new(rpc: &str) -> Self
    where
        Self: Sized,
    {
        Self {
            rpc: rpc.to_string(),
        }
    }
}

impl CallBuilder for AcalaSwap {
    fn build_call(&self, step: Step) -> Result<Vec<Call>, &'static str> {
        let amount_out = Compact(1_u8);
        let amount_in = Compact(step.spend_amount.ok_or("MissingSpendAmount")?);

        let asset0_location: MultiLocation = Decode::decode(&mut step.spend_asset.as_slice())
            .log_err(&format!(
                "AcalaSwap: FailedToScaleDecode, asset: {:?}",
                &step.spend_asset
            ))
            .map_err(|_| "FailedToScaleDecode")?;
        let asset1_location: MultiLocation = Decode::decode(&mut step.receive_asset.as_slice())
            .log_err(&format!(
                "AcalaSwap: FailedToScaleDecode, asset: {:?}",
                &step.receive_asset
            ))
            .map_err(|_| "FailedToScaleDecode")?;

        let token0 =
            AcalaAssetMap::get_currency_id(&asset0_location).ok_or("AssetNotRecognized")?;
        let token1 =
            AcalaAssetMap::get_currency_id(&asset1_location).ok_or("AssetNotRecognized")?;

        // FIXME: hardcode for demo
        if token0 != CurrencyId::Token(TokenSymbol::DOT)
            || token1 != CurrencyId::Token(TokenSymbol::ACA)
        {
            pink_extension::debug!("AcalaDexExecutor: Unsupported trading pair",);
            return Err("Unimplemented");
        }

        let taiga_path = AggregatedSwapPath::Taiga(0, 0, 1);
        // FIXME: Looks like first node is LDOT, represents dex will spend DOT
        let dex_path = AggregatedSwapPath::Dex(vec![
            CurrencyId::Token(TokenSymbol::LDOT),
            CurrencyId::Token(TokenSymbol::AUSD),
            token1,
        ]);
        let path = vec![taiga_path, dex_path];

        let swap_calldata = SubUnsignedExtrinsic {
            // the call index of acala dex module
            //0x5b_u8,
            // the call index of acala aggregateddex module
            pallet_id: 0x5d_u8,
            call_id: 0x0u8,
            call: (path, amount_in, amount_out),
        };

        Ok(vec![Call {
            params: CallParams::Sub(SubCall {
                calldata: swap_calldata.encode(),
            }),
            input_call: None,
            call_index: None,
        }])
    }
}
