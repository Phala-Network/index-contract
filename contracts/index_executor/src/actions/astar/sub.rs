use pink_extension::AccountId;
use xcm::v3::prelude::*;

use super::asset::AstarAssets;
use crate::call::{Call, CallBuilder, CallParams, SubCall, SubExtrinsic};
use crate::step::Step;
use crate::utils::ToArray;
use alloc::{string::String, vec, vec::Vec};
use pink_subrpc::hasher::{Blake2_256, Hasher};
use scale::{Compact, Decode, Encode};

type MultiAddress = sp_runtime::MultiAddress<AccountId, u32>;

#[derive(Clone)]
pub struct AstarSubToEvmTransactor {
    transactor: AstarTransactor,
}

impl AstarSubToEvmTransactor {
    pub fn new(native: Vec<u8>) -> Self
    where
        Self: Sized,
    {
        Self {
            transactor: AstarTransactor::new(native),
        }
    }
}

impl AstarSubToEvmTransactor {
    fn h160_to_sr25519_pub(&self, addr: &[u8]) -> [u8; 32] {
        Blake2_256::hash(&[b"evm:", addr].concat())
    }
}

impl CallBuilder for AstarSubToEvmTransactor {
    fn build_call(&self, step: Step) -> Result<Vec<Call>, &'static str> {
        let bytes: [u8; 20] = step.recipient.clone().ok_or("MissingRecipient")?.to_array();
        let mut new_step = step;
        new_step.recipient = Some(self.h160_to_sr25519_pub(&bytes).to_vec());
        self.transactor.build_call(new_step)
    }
}

#[derive(Clone)]
pub struct AstarTransactor {
    native: Vec<u8>,
}

impl AstarTransactor {
    pub fn new(native: Vec<u8>) -> Self
    where
        Self: Sized,
    {
        Self { native }
    }
}

impl CallBuilder for AstarTransactor {
    fn build_call(&self, step: Step) -> Result<Vec<Call>, &'static str> {
        let asset_location = MultiLocation::decode(&mut step.spend_asset.as_slice())
            .map_err(|_| "FailedToScaleDecode")?;
        let bytes: [u8; 32] = step.recipient.ok_or("MissingRecipient")?.to_array();
        let recipient = MultiAddress::Id(AccountId::from(bytes));
        let amount = Compact(step.spend_amount.ok_or("MissingSpendAmount")?);

        if step.spend_asset == self.native {
            Ok(vec![Call {
                params: CallParams::Sub(SubCall {
                    calldata: SubExtrinsic {
                        // Balance
                        pallet_id: 0x1fu8,
                        call_id: 0x0u8,
                        call: (recipient, amount),
                    }
                    .encode(),
                }),
                input_call: None,
                call_index: None,
            }])
        } else {
            let asset_id = AstarAssets::new()
                .get_assetid(&String::from("Astar"), &asset_location)
                .ok_or("AssetNotFound")?;
            Ok(vec![Call {
                params: CallParams::Sub(SubCall {
                    calldata: SubExtrinsic {
                        // palletAsset
                        pallet_id: 0x24u8,
                        call_id: 0x08u8,
                        call: (Compact(asset_id), recipient, amount),
                    }
                    .encode(),
                }),
                input_call: None,
                call_index: None,
            }])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use pink_subrpc::{create_transaction_with_calldata, send_transaction, ExtraParam};

    #[test]
    #[ignore]
    fn test_astr_transfer() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let astr_location = MultiLocation::new(1, X1(Parachain(2006)));
        let transactor = AstarTransactor::new(astr_location.encode());
        let recipient =
            hex::decode("b63a28ab657209e5894e9021fb680180e2ef9c66ae80a7f6db41f2ed3c9e8707")
                .unwrap();
        let endpoint = "https://astar.public.blastapi.io";

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let calls = transactor
            .build_call(Step {
                exe_type: String::from(""),
                exe: String::from(""),
                source_chain: String::from("Astar"),
                dest_chain: String::from("Astar"),
                spend_asset: astr_location.encode(),
                receive_asset: astr_location.encode(),
                sender: None,
                recipient: Some(recipient),
                // 0.1 ASTR
                spend_amount: Some(1_00_000_000_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();
        match &calls[0].params {
            CallParams::Sub(sub_call) => {
                let signed_tx = create_transaction_with_calldata(
                    &signer,
                    &"Astar",
                    endpoint,
                    &sub_call.calldata,
                    ExtraParam::default(),
                )
                .map_err(|_| "InvalidSignature")
                .unwrap();

                // Live network: https://astar.subscan.io/extrinsic/4269203-6
                let tx_id = send_transaction(&endpoint, &signed_tx)
                    .map_err(|_| "FailedToSubmitTransaction")
                    .unwrap();
                println!("Astar asset transfer: {}", &hex::encode(&tx_id));
            }
            _ => {
                assert!(false);
            }
        }
    }

    #[test]
    #[ignore]
    fn test_pha_transfer() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let astr_location = MultiLocation::new(1, X1(Parachain(2006)));
        let pha_location = MultiLocation::new(1, X1(Parachain(2035)));
        let transactor = AstarTransactor::new(astr_location.encode());
        let recipient =
            hex::decode("b63a28ab657209e5894e9021fb680180e2ef9c66ae80a7f6db41f2ed3c9e8707")
                .unwrap();
        let endpoint = "https://astar.public.blastapi.io";

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let calls = transactor
            .build_call(Step {
                exe_type: String::from(""),
                exe: String::from(""),
                source_chain: String::from("Astar"),
                dest_chain: String::from("Astar"),
                spend_asset: pha_location.encode(),
                receive_asset: pha_location.encode(),
                sender: None,
                recipient: Some(recipient),
                // 0.1 PHA
                spend_amount: Some(1_00_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();
        match &calls[0].params {
            CallParams::Sub(sub_call) => {
                let signed_tx = create_transaction_with_calldata(
                    &signer,
                    &"Astar",
                    endpoint,
                    &sub_call.calldata,
                    ExtraParam::default(),
                )
                .map_err(|_| "InvalidSignature")
                .unwrap();

                // Live network: https://astar.subscan.io/extrinsic/4269671-5
                let tx_id = send_transaction(&endpoint, &signed_tx)
                    .map_err(|_| "FailedToSubmitTransaction")
                    .unwrap();
                println!("Astar asset transfer: {}", &hex::encode(&tx_id));
            }
            _ => {
                assert!(false);
            }
        }
    }

    #[test]
    #[ignore]
    fn test_pha_to_evm_transfer() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let astr_location = MultiLocation::new(1, X1(Parachain(2006)));
        let pha_location = MultiLocation::new(1, X1(Parachain(2035)));
        let transactor = AstarSubToEvmTransactor::new(astr_location.encode());
        let h160_recipient = hex::decode("e887376a93bDa91ed66D814528D7aeEfe59990a5").unwrap();
        let endpoint = "https://astar.public.blastapi.io";

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let calls = transactor
            .build_call(Step {
                exe_type: String::from("bridge"),
                exe: String::from(""),
                source_chain: String::from("Astar"),
                dest_chain: String::from("AstarEvm"),
                spend_asset: pha_location.encode(),
                receive_asset: pha_location.encode(),
                sender: None,
                recipient: Some(h160_recipient),
                // 0.1 PHA
                spend_amount: Some(1_00_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();
        match &calls[0].params {
            CallParams::Sub(sub_call) => {
                let signed_tx = create_transaction_with_calldata(
                    &signer,
                    &"Astar",
                    endpoint,
                    &sub_call.calldata,
                    ExtraParam::default(),
                )
                .map_err(|_| "InvalidSignature")
                .unwrap();

                // Live network: https://astar.subscan.io/extrinsic/4270220-4
                let tx_id = send_transaction(&endpoint, &signed_tx)
                    .map_err(|_| "FailedToSubmitTransaction")
                    .unwrap();
                println!("Astar asset to EVM transfer: {}", &hex::encode(&tx_id));
            }
            _ => {
                assert!(false);
            }
        }
    }
}
