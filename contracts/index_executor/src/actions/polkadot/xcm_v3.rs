use crate::account::AccountType;
use crate::call::{Call, CallBuilder, CallParams, SubCall, SubExtrinsic};
use crate::step::Step;
use crate::utils::{h160_to_sr25519_pub, ToArray};
use alloc::vec;
use scale::{Decode, Encode};
use xcm::{v3::prelude::*, VersionedMultiAssets, VersionedMultiLocation};

#[derive(Clone)]
pub struct PolkadotXcm {
    dest_chain_id: u32,
    account_type: AccountType,
    is_evm: bool,
}

impl PolkadotXcm {
    pub fn new(dest_chain_id: u32, account_type: AccountType, is_evm: bool) -> Self
    where
        Self: Sized,
    {
        Self {
            dest_chain_id,
            account_type,
            is_evm,
        }
    }
}

impl CallBuilder for PolkadotXcm {
    fn build_call(&self, step: Step) -> Result<Call, &'static str> {
        let recipient = step.recipient;
        let asset_location: MultiLocation =
            Decode::decode(&mut step.spend_asset.as_slice()).map_err(|_| "InvalidMultilocation")?;
        let dest = VersionedMultiLocation::V3(MultiLocation::new(
            0,
            Junctions::X1(Parachain(self.dest_chain_id)),
        ));
        let beneficiary = VersionedMultiLocation::V3(MultiLocation::new(
            0,
            Junctions::X1(match &self.account_type {
                AccountType::Account20 => AccountKey20 {
                    network: None,
                    key: recipient.to_array(),
                },
                AccountType::Account32 => AccountId32 {
                    network: None,
                    id: match self.is_evm {
                        true => h160_to_sr25519_pub(&recipient),
                        false => recipient.to_array(),
                    },
                },
            }),
        ));
        let assets = VersionedMultiAssets::V3(MultiAssets::from(vec![MultiAsset {
            id: AssetId::Concrete(asset_location),
            fun: Fungibility::Fungible(step.spend_amount.ok_or("MissingSpendAmount")?),
        }]));
        let fee_asset_item: u32 = 0;

        Ok(Call {
            params: CallParams::Sub(SubCall {
                calldata: SubExtrinsic {
                    pallet_id: 0x63u8,
                    call_id: 0x02u8,
                    call: (dest, beneficiary, assets, fee_asset_item),
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
    use crate::constants::ASTAR_PARACHAIN_ID;

    #[test]
    fn test_bridge_to_astar_evm() {
        let xcm = PolkadotXcm {
            dest_chain_id: ASTAR_PARACHAIN_ID,
            account_type: AccountType::Account32,
            is_evm: true,
        };
        let call = xcm
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Polkadot"),
                dest_chain: String::from("AstarEvm"),
                spend_asset: hex::decode("0000").unwrap(),
                receive_asset: hex::decode("FFfFfFffFFfffFFfFFfFFFFFffFFFffffFfFFFfF").unwrap(),
                sender: None,
                recipient: hex::decode("bEA1C40ecf9c4603ec25264860B9b6623Ff733F5").unwrap(),
                // 1.1 DOT
                spend_amount: Some(11_000_000_000 as u128),
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

    #[test]
    fn test_bridge_to_astar() {
        let xcm = PolkadotXcm {
            dest_chain_id: ASTAR_PARACHAIN_ID,
            account_type: AccountType::Account32,
            is_evm: false,
        };
        let call = xcm
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Polkadot"),
                dest_chain: String::from("Astar"),
                spend_asset: hex::decode("0000").unwrap(),
                receive_asset: hex::decode("0100").unwrap(),
                sender: None,
                recipient: hex::decode(
                    "04dba0677fc274ffaccc0fa1030a66b171d1da9226d2bb9d152654e6a746f276",
                )
                .unwrap(),
                // 1.1 DOT
                spend_amount: Some(11_000_000_000 as u128),
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
