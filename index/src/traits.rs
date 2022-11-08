use primitive_types::{H160, H256, U256};
use scale::{Decode, Encode};

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Error {
    BadAbi,
    InvalidAddress,
    InvalidBody,
    SubRPCRequestFailed,
    Ss58,
    InvalidSignature,
    InvalidAmount,
    InvalidMultilocation,
}

pub enum Address {
    EthAddr(H160),
    SubAddr(H256),
}

pub enum Amount {
    U256(U256),
    U128(u128),
}

pub trait Executor {
    fn new(
        bridge_address: Address,
        abi_json: &[u8],
        rpc: &str,
    ) -> core::result::Result<Self, Error>
    where
        Self: Sized;
    fn transfer(
        &self,
        signer: [u8; 32], // FIXME
        token_rid: H256,
        amount: Amount,
        recipient: Address,
    ) -> core::result::Result<(), Error>;
}
