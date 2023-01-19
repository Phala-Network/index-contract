use pink_subrpc as subrpc;

use crate::traits::{common::Error, executor::BridgeExecutor};
use crate::transactors::ChainBridgeClient;
use crate::utils::ToArray;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use pink_web3::{
    api::{Eth, Namespace},
    contract::Contract,
    keys::pink::KeyPair,
    transports::PinkHttp,
    types::Address,
};
use primitive_types::U256;
use scale::Decode;
use scale::Encode;
use subrpc::{create_transaction, send_transaction, ExtraParam};
use xcm::v1::{prelude::*, AssetId, Fungibility, Junction, Junctions, MultiAsset, MultiLocation};

pub mod moonbeam_to_acala;
pub mod moonbeam_to_phala;
pub mod phala_to_acala;

#[derive(Clone)]
pub struct ChainBridgeEvm2Phala {
    // (asset_contract_address, resource_id)
    assets: Vec<(Address, [u8; 32])>,
    bridge_contract: ChainBridgeClient,
}

#[allow(dead_code)]
impl ChainBridgeEvm2Phala {
    pub fn new(rpc: &str, bridge_address: Address, assets: Vec<(Address, [u8; 32])>) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let bridge_contract = ChainBridgeClient {
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
impl BridgeExecutor for ChainBridgeEvm2Phala {
    fn transfer(
        &self,
        signer: [u8; 32],
        asset: Vec<u8>,
        recipient: Vec<u8>,
        amount: u128,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let signer = KeyPair::from(signer);
        let recipient: [u8; 32] = recipient.try_into().expect("Invalid recipient");
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

#[derive(Clone)]
pub struct ChainBridgePhala2Evm {
    evm_chainid: u8,
    rpc: String,
}

#[allow(dead_code)]
impl ChainBridgePhala2Evm {
    pub fn new(evm_chainid: u8, rpc: &str) -> Self
    where
        Self: Sized,
    {
        Self {
            evm_chainid,
            rpc: rpc.to_string(),
        }
    }
}

#[allow(dead_code)]
impl BridgeExecutor for ChainBridgePhala2Evm {
    fn transfer(
        &self,
        signer: [u8; 32],
        asset: Vec<u8>,
        recipient: Vec<u8>,
        amount: u128,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let asset_location: MultiLocation =
            Decode::decode(&mut asset.as_slice()).map_err(|_| Error::InvalidMultilocation)?;
        let multi_asset = MultiAsset {
            id: AssetId::Concrete(asset_location),
            fun: Fungibility::Fungible(amount),
        };
        let dest = MultiLocation::new(
            0,
            Junctions::X3(
                Junction::GeneralKey(
                    b"cb"
                        .to_vec()
                        .try_into()
                        .or(Err(Error::InvalidMultilocation))?,
                ),
                Junction::GeneralIndex(self.evm_chainid as u128),
                Junction::GeneralKey(recipient.try_into().or(Err(Error::InvalidMultilocation))?),
            ),
        );
        let dest_weight: core::option::Option<u64> = None;
        let signed_tx = create_transaction(
            &signer,
            "phala",
            &self.rpc,
            0x52u8,
            0x0u8,
            (multi_asset, dest, dest_weight),
            extra,
        )
        .map_err(|_| Error::InvalidSignature)?;
        let tx_id =
            send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)?;
        Ok(tx_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::H256;
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
