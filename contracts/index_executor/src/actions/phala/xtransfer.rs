use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use scale::{Decode, Encode};

use crate::call::{Call, CallBuilder, CallParams, SubCall, SubExtrinsic};
use crate::step::Step;

use crate::utils::ToArray;
use xcm::v3::{prelude::*, AssetId, Fungibility, Junctions, MultiAsset, MultiLocation};

use crate::account::AccountType;

#[derive(Clone)]
pub struct XTransferXcm {
    rpc: String,
    dest_chain_id: u32,
    // dest chain account type
    account_type: AccountType,
}

impl XTransferXcm {
    pub fn new(rpc: &str, dest_chain_id: u32, account_type: AccountType) -> Self
    where
        Self: Sized,
    {
        Self {
            rpc: rpc.to_string(),
            dest_chain_id,
            account_type,
        }
    }
}

impl CallBuilder for XTransferXcm {
    fn build_call(&self, step: Step) -> Result<Vec<Call>, &'static str> {
        let recipient = step.recipient.ok_or("MissingRecipient")?;
        let asset_location: MultiLocation =
            Decode::decode(&mut step.spend_asset.as_slice()).map_err(|_| "InvalidMultilocation")?;
        let multi_asset = MultiAsset {
            id: AssetId::Concrete(asset_location),
            fun: Fungibility::Fungible(step.spend_amount.ok_or("MissingSpendAmount")?),
        };
        let dest = MultiLocation::new(
            1,
            Junctions::X2(
                Parachain(self.dest_chain_id),
                match &self.account_type {
                    AccountType::Account20 => {
                        let recipient: [u8; 20] = recipient.to_array();
                        AccountKey20 {
                            network: None,
                            key: recipient,
                        }
                    }
                    AccountType::Account32 => {
                        let recipient: [u8; 32] = recipient.to_array();
                        AccountId32 {
                            network: None,
                            id: recipient,
                        }
                    }
                },
            ),
        );
        let dest_weight: core::option::Option<u64> = Some(6000000000);

        Ok(vec![Call {
            params: CallParams::Sub(SubCall {
                calldata: SubExtrinsic {
                    // the call index of acala dex module
                    //0x5b_u8,
                    // the call index of acala aggregateddex module
                    pallet_id: 0x52u8,
                    call_id: 0x0u8,
                    call: (multi_asset, dest, dest_weight),
                }
                .encode(),
            }),
            input_call: None,
            call_index: None,
        }])
    }
}
