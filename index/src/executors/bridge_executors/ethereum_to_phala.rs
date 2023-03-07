use pink_subrpc as subrpc;

use crate::traits::{common::Error, executor::BridgeExecutor};
use crate::transactors::ChainBridgeDepositer;
use crate::utils::ToArray;
use alloc::vec::Vec;
use pink_web3::{
    api::{Eth, Namespace},
    contract::Contract,
    keys::pink::KeyPair,
    transports::PinkHttp,
    types::Address,
};
use primitive_types::U256;
use scale::Encode;
use subrpc::ExtraParam;
use xcm::v1::{prelude::*, Junction, Junctions, MultiLocation};

#[derive(Clone)]
pub struct ChainBridgeEthereum2Phala {
    // ((asset_contract_address, dest_chain), resource_id)
    assets: Vec<(Address, [u8; 32])>,
    bridge_contract: ChainBridgeDepositer,
}

#[allow(dead_code)]
impl ChainBridgeEthereum2Phala {
    pub fn new(rpc: &str, bridge_address: Address, assets: Vec<(Address, [u8; 32])>) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let bridge_contract = ChainBridgeDepositer {
            contract: Contract::from_json(
                eth,
                bridge_address,
                include_bytes!("../../abis/chainbridge-abi.json"),
            )
            .expect("Bad abi data"),
        };

        Self {
            assets,
            bridge_contract,
        }
    }

    fn lookup_rid(&self, addr: Address) -> Option<[u8; 32]> {
        self.assets
            .iter()
            .position(|a| a.0 == addr)
            .map(|idx| self.assets[idx].1)
    }
}

#[allow(dead_code)]
impl BridgeExecutor for ChainBridgeEthereum2Phala {
    fn transfer(
        &self,
        signer: [u8; 32],
        asset: Vec<u8>,
        recipient: Vec<u8>,
        amount: u128,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let signer = KeyPair::from(signer);
        let recipient: [u8; 32] = recipient.try_into().map_err(|_| Error::InvalidAddress)?;
        let dest = MultiLocation::new(
            0,
            Junctions::X1(Junction::AccountId32 {
                network: NetworkId::Any,
                id: recipient,
            }),
        );
        let asset: [u8; 20] = asset.to_array();
        let rid = self
            .lookup_rid(asset.into())
            .ok_or(Error::InvalidMultilocation)?;
        let tx_id = self.bridge_contract.deposit(
            signer,
            rid.into(),
            U256::from(amount),
            dest.encode(),
            extra.nonce,
        )?;
        Ok(tx_id.as_bytes().to_vec())
    }
}
