use crate::traits::{
    common::{Address, Error},
    executor::Executor,
};
use crate::transactors::ChainBridgeClient;
use pink_web3::api::{Eth, Namespace};
use pink_web3::contract::Contract;
use pink_web3::keys::pink::KeyPair;
use pink_web3::transports::PinkHttp;
use primitive_types::{H256, U256};
use scale::Encode;
use xcm::v0::NetworkId;
use xcm::v1::{Junction, Junctions, MultiLocation};

pub struct Evm2PhalaExecutor {
    bridge_contract: ChainBridgeClient,
}

impl Executor for Evm2PhalaExecutor {
    fn new(
        bridge_address: Address,
        abi_json: &[u8],
        rpc: &str,
    ) -> core::result::Result<Self, Error> {
        let eth = Eth::new(PinkHttp::new(rpc));
        if let Address::EthAddr(address) = bridge_address {
            Ok(Self {
                bridge_contract: ChainBridgeClient {
                    contract: Contract::from_json(eth, address, abi_json).or(Err(Error::BadAbi))?,
                },
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
        recipient: Address,
    ) -> core::result::Result<(), Error> {
        let signer = KeyPair::from(signer);
        match recipient {
            Address::SubAddr(addr) => {
                let dest = MultiLocation::new(
                    0,
                    Junctions::X1(Junction::AccountId32 {
                        network: NetworkId::Any,
                        id: addr.into(),
                    }),
                );
                _ = self
                    .bridge_contract
                    .deposit(signer, token_rid, amount, dest.encode())?;
                Ok(())
            }
            _ => Err(Error::InvalidAddress),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale::Encode;
    #[test]
    fn it_works() {
        use hex_literal::hex;
        let recipient: Vec<u8> =
            hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48").into();
        let addr: H256 = H256::from_slice(&recipient);
        let dest = MultiLocation::new(
            0,
            Junctions::X1(Junction::AccountId32 {
                network: NetworkId::Any,
                id: addr.into(),
            }),
        );
        let expected: Vec<u8> =
            hex!("000101008eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48").into();
        assert_eq!(dest.encode(), expected);
    }
}
