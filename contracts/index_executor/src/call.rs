use crate::step::Step;
use alloc::vec::Vec;
use dyn_clone::DynClone;
use pink_web3::{
    contract::{
        tokens::{Tokenizable, TokenizableItem},
        Error as PinkError,
    },
    ethabi::Token,
    types::{Address, Bytes, U256},
};
use scale::{Decode, Encode};

#[derive(Clone, Debug)]
pub struct EvmCall {
    // The call metadata
    pub target: Address,
    pub calldata: Bytes,
    pub value: U256,

    pub need_settle: bool,
    pub update_offset: U256,
    pub update_len: U256,
    pub spend_asset: Address,
    pub spend_amount: U256,
    pub receive_asset: Address,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SubCall {
    pub calldata: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SubUnsignedExtrinsic<T: Encode> {
    pub pallet_id: u8,
    pub call_id: u8,
    pub call: T,
}

#[derive(Clone, Debug)]
pub enum CallParams {
    Evm(EvmCall),
    Sub(SubCall),
}

#[derive(Clone, Debug)]
pub struct Call {
    pub params: CallParams,
    // The call index that whose result will be the input of call
    pub input_call: Option<u8>,
    // Current call index
    pub call_index: Option<u8>,
}

impl Tokenizable for Call {
    fn from_token(_token: Token) -> Result<Self, PinkError> {
        Err(PinkError::InterfaceUnsupported)
    }

    fn into_token(self) -> Token {
        let mut tokens: Vec<Token> = vec![];
        match (self.params, self.input_call, self.call_index) {
            (CallParams::Evm(evm_call), Some(input_call), Some(call_index)) => {
                tokens.push(evm_call.target.into_token());
                tokens.push(evm_call.calldata.into_token());
                tokens.push(evm_call.value.into_token());
                tokens.push(evm_call.need_settle.into_token());
                tokens.push(evm_call.update_offset.into_token());
                tokens.push(evm_call.update_len.into_token());
                tokens.push(evm_call.spend_asset.into_token());
                tokens.push(evm_call.spend_amount.into_token());
                tokens.push(evm_call.receive_asset.into_token());
                tokens.push(U256::from(input_call).into_token());
                tokens.push(U256::from(call_index).into_token());
            }
            _ => {
                return Token::Tuple(vec![]);
            }
        }
        Token::Tuple(tokens)
    }
}

impl TokenizableItem for Call {}

pub trait CallBuilder: DynClone {
    fn build_call(&self, step: Step) -> Result<Vec<Call>, &'static str>;
}
dyn_clone::clone_trait_object!(CallBuilder);
