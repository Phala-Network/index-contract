use crate::step::Step;
use alloc::{vec, vec::Vec};
use dyn_clone::DynClone;
use pink_web3::types::{Address, U256};
use scale::{Decode, Encode};

pub trait PackCall {
    fn pack(self) -> Vec<u8>;
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct EvmCall {
    // The call metadata
    pub target: Address,
    pub calldata: Vec<u8>,
    pub value: U256,

    pub need_settle: bool,
    pub update_offset: u16,
    pub update_len: u16,
    pub spender: Address,
    pub spend_asset: Address,
    pub spend_amount: U256,
    pub receive_asset: Address,
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SubExtrinsic<T: Encode> {
    pub pallet_id: u8,
    pub call_id: u8,
    pub call: T,
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SubCall {
    pub calldata: Vec<u8>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CallParams {
    Evm(EvmCall),
    Sub(SubCall),
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Call {
    pub params: CallParams,
    // The call index that whose result will be the input of call
    pub input_call: Option<u8>,
    // Current call index
    pub call_index: Option<u8>,
}

impl PackCall for Call {
    fn pack(self) -> Vec<u8> {
        let mut output: Vec<u8> = vec![];
        match (self.params, self.input_call, self.call_index) {
            (CallParams::Evm(evm_call), Some(input_call), Some(call_index)) => {
                // target
                output.extend_from_slice(evm_call.target.as_bytes());
                // calldata length
                output.extend_from_slice(
                    &TryInto::<u16>::try_into(evm_call.calldata.len())
                        .expect("Exceeds calldata limit")
                        .to_be_bytes(),
                );
                // calldata
                output.extend_from_slice(&evm_call.calldata);
                // value
                output.extend_from_slice(&{
                    let mut res = Vec::new();
                    for b in evm_call.value.0.iter().rev() {
                        let bytes = b.to_be_bytes();
                        res.extend(bytes);
                    }
                    res
                });
                // needSettle
                output.extend_from_slice(&[if evm_call.need_settle { 1 } else { 0 }]);
                // updateOffset
                output.extend_from_slice(&evm_call.update_offset.to_le_bytes());
                // updateLen
                output.extend_from_slice(&evm_call.update_len.to_le_bytes());
                // spender
                output.extend_from_slice(evm_call.spender.as_bytes());
                // spendAsset
                output.extend_from_slice(evm_call.spend_asset.as_bytes());
                // spendAmount
                output.extend_from_slice(&{
                    let mut res = Vec::new();
                    for b in evm_call.spend_amount.0.iter().rev() {
                        let bytes = b.to_be_bytes();
                        res.extend(bytes);
                    }
                    res
                });
                // receiveAsset
                output.extend_from_slice(evm_call.receive_asset.as_bytes());
                // inputCall
                output.extend_from_slice(&[input_call]);
                // callIndex
                output.extend_from_slice(&[call_index]);
            }
            _ => None.expect("Illegal Call"),
        }
        output
    }
}

impl PackCall for Vec<Call> {
    fn pack(self) -> Vec<u8> {
        let mut output = vec![];
        output.push(TryInto::<u8>::try_into(self.len()).expect("Exceeds call limit"));

        for call in self.iter() {
            let mut encoded_call: Vec<u8> = call.clone().pack();
            output.append(&mut encoded_call)
        }
        output
    }
}

pub trait CallBuilder: DynClone {
    fn build_call(&self, step: Step) -> Result<Call, &'static str>;
}
dyn_clone::clone_trait_object!(CallBuilder);

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::*;
    #[test]
    fn pack_call_should_work() {
        let call = Call {
            params: CallParams::Evm(EvmCall {
                target: Address::from_str("Cf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9").unwrap(),
                calldata: hex::decode(
                    "dce1d5ba0000000000000000000000000000000000000000000000000000000000002710",
                )
                .unwrap(),
                value: U256::from(0),

                need_settle: false,
                update_offset: 0,
                update_len: 0,
                spender: Address::from_str("0000000000000000000000000000000000000000").unwrap(),
                spend_asset: Address::from_str("5FbDB2315678afecb367f032d93F642f64180aa3").unwrap(),
                spend_amount: U256::from(0),
                receive_asset: Address::from_str("5FbDB2315678afecb367f032d93F642f64180aa3")
                    .unwrap(),
            }),
            input_call: Some(0),
            call_index: Some(0),
        };

        assert_eq!(vec![call].pack(), hex::decode("01Cf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc90024dce1d5ba00000000000000000000000000000000000000000000000000000000000027100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005FbDB2315678afecb367f032d93F642f64180aa300000000000000000000000000000000000000000000000000000000000000005FbDB2315678afecb367f032d93F642f64180aa30000").unwrap())
    }
}
