use pink_extension::AccountId;
use scale::{Compact, Encode};

use crate::call::{Call, CallBuilder, CallParams, SubCall, SubExtrinsic};
use crate::step::Step;
use crate::utils::ToArray;

type MultiAddress = sp_runtime::MultiAddress<AccountId, u32>;

#[derive(Clone)]
pub struct Transactor {
    pallet_id: u8,
    call_id: u8,
}

impl Transactor {
    pub fn new(pallet_id: u8, call_id: u8) -> Self
    where
        Self: Sized,
    {
        Self { pallet_id, call_id }
    }
}

impl CallBuilder for Transactor {
    fn build_call(&self, step: Step) -> Result<Call, &'static str> {
        let dest = match step.recipient.len() {
            20 => MultiAddress::Address20(step.recipient.to_array()),
            32 => MultiAddress::Id(AccountId::from(step.recipient.to_array())),
            _ => return Err("InvalidRecipient"),
        };
        let value = Compact(step.spend_amount.ok_or("MissingSpendAmount")?);

        Ok(Call {
            params: CallParams::Sub(SubCall {
                calldata: SubExtrinsic {
                    pallet_id: self.pallet_id,
                    call_id: self.call_id,
                    call: (dest, value),
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

    #[test]
    fn transfer_on_phala() {
        let transactor = Transactor::new(0x28, 0x07);
        let call = transactor
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Phala"),
                dest_chain: String::from("Phala"),
                spend_asset: hex::decode("0000").unwrap(),
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
