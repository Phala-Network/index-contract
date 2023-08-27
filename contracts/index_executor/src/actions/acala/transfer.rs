use pink_extension::AccountId;
use xcm::v3::prelude::*;

use super::asset::{AcalaAssets, CurrencyId, TokenType as AcalaTokenType};
use crate::call::{Call, CallBuilder, CallParams, SubCall, SubExtrinsic};
use crate::step::Step;
use crate::utils::ToArray;
use alloc::{vec, vec::Vec};
use scale::{Compact, Decode, Encode};

type MultiAddress = sp_runtime::MultiAddress<AccountId, u32>;

#[derive(Clone)]
pub struct AcalaTransactor;

impl AcalaTransactor {
    pub fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }
}

impl CallBuilder for AcalaTransactor {
    fn build_call(&self, step: Step) -> Result<Vec<Call>, &'static str> {
        let asset_location = MultiLocation::decode(&mut step.spend_asset.as_slice())
            .map_err(|_| "FailedToScaleDecode")?;
        let bytes: [u8; 32] = step.recipient.ok_or("MissingRecipient")?.to_array();
        let recipient = MultiAddress::Id(AccountId::from(bytes));
        let asset_attrs = AcalaAssets::get_asset_attrs(&asset_location).ok_or("BadAsset")?;
        let currency_id = CurrencyId::Token(asset_attrs.0);
        let asset_type = asset_attrs.1;
        let amount = Compact(step.spend_amount.ok_or("MissingSpendAmount")?);

        match asset_type {
            AcalaTokenType::Utility => {
                Ok(vec![Call {
                    params: CallParams::Sub(SubCall {
                        calldata: SubExtrinsic {
                            // Balance
                            pallet_id: 0x0au8,
                            call_id: 0x0u8,
                            call: (recipient, amount),
                        }
                        .encode(),
                    }),
                    input_call: None,
                    call_index: None,
                }])
            }
            _ => {
                let currency_id = match asset_type {
                    AcalaTokenType::Foreign => {
                        let foreign_asset_id = asset_attrs.2.ok_or("BadAsset")?;
                        CurrencyId::ForeignAsset(foreign_asset_id)
                    }
                    _ => currency_id,
                };
                Ok(vec![Call {
                    params: CallParams::Sub(SubCall {
                        calldata: SubExtrinsic {
                            // Currencies
                            pallet_id: 0x0cu8,
                            call_id: 0x0u8,
                            call: (recipient, currency_id, amount),
                        }
                        .encode(),
                    }),
                    input_call: None,
                    call_index: None,
                }])
            }
        }
    }
}

#[cfg(test)]
mod tests {}
