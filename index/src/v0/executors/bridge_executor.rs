use super::evm_transactor::{self, EvmContractClient};
use crate::v0::traits::{Address, Error, Executor};
use pink_web3::api::{Eth, Namespace};
use pink_web3::contract::Contract;
use pink_web3::keys::pink::KeyPair;
use pink_web3::transports::{resolve_ready, PinkHttp};
use primitive_types::{H160, H256, U256};
use alloc::vec::Vec;

pub struct Evm2PhalaExecutor {
    bridge_contract: EvmContractClient,
    src_chain: u8,
    dst_chain: u8,
}

impl Executor for Evm2PhalaExecutor {
    fn new(
        bridge_address: Address,
        abi_json: &[u8],
        rpc: &str,
        src_chain: u8,
        dst_chain: u8,
    ) -> core::result::Result<Self, Error> {
        let eth = Eth::new(PinkHttp::new(rpc));
        if let Address::EthAddr(address) = bridge_address {
            Ok(Self {
                bridge_contract: EvmContractClient {
                    contract: Contract::from_json(eth, address, abi_json).or(Err(Error::BadAbi))?,
                },
                src_chain,
                dst_chain,
            })
        } else {
            Err(Error::InvalidAddress)
        }
    }

    fn transfer(
        &self,
        signer: [u8; 32],
        token_rid: H256,
        amount: U256,
    ) -> core::result::Result<(), Error> {
        use hex_literal::hex;
        let signer = KeyPair::from(signer);
        // todo: to derive the recipient address here
        // FIXME: the recipient address have something to do with the subbridge
        // for now we just concatenate `0x00010100` to a hardcoded one
        let recipient_address: Vec<u8> =
            hex!("000101008eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48").into();
        _ = self.bridge_contract.deposit(
            signer,
            self.dst_chain,
            token_rid,
            amount,
            recipient_address,
        )?;
        Ok(())
    }
}
