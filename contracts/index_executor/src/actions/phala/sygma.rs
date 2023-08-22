use alloc::{vec, vec::Vec};
use scale::{Decode, Encode};

use crate::call::{Call, CallBuilder, CallParams, SubCall, SubExtrinsic};
use crate::step::Step;

use crate::utils::ToArray;
use xcm::v3::{prelude::*, AssetId, Fungibility, Junctions, MultiAsset, MultiLocation, Weight};

use crate::utils::slice_to_generalkey;

#[derive(Clone)]
pub struct XTransferSygma {
    evm_domain_id: u8,
}

impl XTransferSygma {
    pub fn new(evm_domain_id: u8) -> Self
    where
        Self: Sized,
    {
        Self { evm_domain_id }
    }
}

impl CallBuilder for XTransferSygma {
    fn build_call(&self, step: Step) -> Result<Vec<Call>, &'static str> {
        let recipient: [u8; 32] = step.recipient.ok_or("MissingRecipient")?.to_array();
        let asset_location: MultiLocation =
            Decode::decode(&mut step.spend_asset.as_slice()).map_err(|_| "InvalidMultilocation")?;
        let multi_asset = MultiAsset {
            id: AssetId::Concrete(asset_location),
            fun: Fungibility::Fungible(step.spend_amount.ok_or("MissingSpendAmount")?),
        };
        let dest = MultiLocation::new(
            0,
            Junctions::X3(
                slice_to_generalkey("sygma".as_bytes()),
                GeneralIndex(self.evm_domain_id as u128),
                slice_to_generalkey(&recipient),
            ),
        );
        let dest_weight: Option<Weight> = None;

        Ok(vec![Call {
            params: CallParams::Sub(SubCall {
                calldata: SubExtrinsic {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{call::CallBuilder, constants::SYGMA_ETHEREUM_DOMAIN_ID, step::Step};
    use dotenv::dotenv;
    use pink_subrpc::{create_transaction_with_calldata, send_transaction, ExtraParam};
    use xcm::v3::{Junctions, MultiLocation};

    #[test]
    fn test_pha_from_phala_to_ethereum() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let xtransfer_sygma_ethereum = XTransferSygma::new(SYGMA_ETHEREUM_DOMAIN_ID);
        let pha_location = MultiLocation::new(0, Junctions::Here);
        let recipient: Vec<u8> =
            hex_literal::hex!("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").into();
        let endpoint = "https://subbridge-test.phala.network/rhala/ws";

        let calls = xtransfer_sygma_ethereum
            .build_call(Step {
                exe_type: String::from(""),
                exe: String::from(""),
                source_chain: String::from("Phala"),
                dest_chain: String::from("Ethereum"),
                spend_asset: pha_location.encode(),
                receive_asset: pha_location.encode(),
                sender: None,
                recipient: Some(recipient),
                // Spend 1.1 PHA, 0.1 as fee, expect to receive 1 PHA
                spend_amount: Some(1_100_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        match &calls[0].params {
            CallParams::Sub(sub_call) => {
                let signed_tx = create_transaction_with_calldata(
                    &signer,
                    &"Phala",
                    endpoint,
                    &sub_call.calldata,
                    ExtraParam::default(),
                )
                .map_err(|_| "InvalidSignature")
                .unwrap();

                println!("{:?}", hex::encode(&sub_call.calldata));

                let tx_id = send_transaction(&endpoint, &signed_tx)
                    .map_err(|_| "FailedToSubmitTransaction")
                    .unwrap();
                println!("Phala asset transfer: {}", &hex::encode(&tx_id));
            }
            _ => {
                assert!(false);
            }
        }
    }
}
