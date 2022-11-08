use crate::traits::{Address, Error, Executor};
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