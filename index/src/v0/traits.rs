use primitive_types::{H160, H256, U256};
use scale::{Decode, Encode};

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Error {
    BadAbi,
    InvalidAddress,
}

pub enum Address {
    EthAddr(H160),
    SubAddr(H256),
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
        amount: U256,
    ) -> core::result::Result<(), Error>;
}
