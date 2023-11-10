use crate::call::{Call, CallBuilder, CallParams, SubCall, SubExtrinsic};
use crate::step::Step;
use crate::utils::ToArray;
use scale::{Decode, Encode};
use xcm::{v3::prelude::*, VersionedMultiAsset, VersionedMultiLocation};

#[derive(Clone)]
pub struct AstarXtokens {
    dest_chain_id: u32,
}

impl AstarXtokens {
    pub fn new(dest_chain_id: u32) -> Self
    where
        Self: Sized,
    {
        Self { dest_chain_id }
    }
}

impl CallBuilder for AstarXtokens {
    fn build_call(&self, step: Step) -> Result<Call, &'static str> {
        let asset_location: MultiLocation =
            Decode::decode(&mut step.spend_asset.as_slice()).map_err(|_| "InvalidMultilocation")?;
        let multi_asset = VersionedMultiAsset::V3(MultiAsset {
            id: AssetId::Concrete(asset_location),
            fun: Fungibility::Fungible(step.spend_amount.ok_or("MissingSpendAmount")?),
        });
        let dest = VersionedMultiLocation::V3(MultiLocation::new(
            1,
            Junctions::X2(
                Parachain(self.dest_chain_id),
                match step.recipient.len() {
                    20 => AccountKey20 {
                        network: None,
                        key: step.recipient.to_array(),
                    },
                    32 => AccountId32 {
                        network: None,
                        id: step.recipient.to_array(),
                    },
                    _ => return Err("InvalidRecipient"),
                },
            ),
        ));
        let dest_weight: WeightLimit = WeightLimit::Unlimited;

        Ok(Call {
            params: CallParams::Sub(SubCall {
                calldata: SubExtrinsic {
                    pallet_id: 0x37u8,
                    call_id: 0x01u8,
                    call: (multi_asset, dest, dest_weight),
                }
                .encode(),
            }),
            input_call: None,
            call_index: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::PHALA_PARACHAIN_ID;

    #[test]
    fn test_bridge_to_phala() {
        let xtokens = AstarXtokens {
            dest_chain_id: PHALA_PARACHAIN_ID,
        };
        let call = xtokens
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Astar"),
                dest_chain: String::from("Phala"),
                spend_asset: hex::decode("010100cd1f").unwrap(),
                receive_asset: hex::decode("0000").unwrap(),
                sender: None,
                recipient: hex::decode(
                    "04dba0677fc274ffaccc0fa1030a66b171d1da9226d2bb9d152654e6a746f276",
                )
                .unwrap(),
                // 2 PHA
                spend_amount: Some(2_000_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        match &call.params {
            CallParams::Sub(sub_call) => {
                println!("calldata: {:?}", hex::encode(&sub_call.calldata))
            }
            _ => assert!(false),
        }
    }
}
