use crate::prelude::Executor;

pub struct Phala2EvmExecutor {
    rpc: String,
}

impl Executor for Phala2EvmExecutor {
    fn new(
        bridge_address: crate::traits::Address,
        abi_json: &[u8],
        rpc: &str,
    ) -> core::result::Result<Self, crate::traits::Error>
    where
        Self: Sized,
    {
        Ok(Self { rpc: rpc.into() })
    }

    fn transfer(
        &self,
        signer: [u8; 32],
        token_rid: primitive_types::H256,
        amount: primitive_types::U256,
        recipient: crate::traits::Address,
    ) -> core::result::Result<(), crate::traits::Error> {
        
    }
}
