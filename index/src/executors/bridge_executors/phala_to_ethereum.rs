use pink_subrpc as subrpc;

use crate::traits::{common::Error, executor::BridgeExecutor};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use scale::Decode;
use subrpc::{create_transaction, send_transaction, ExtraParam};
use xcm::v1::{AssetId, Fungibility, Junction, Junctions, MultiAsset, MultiLocation};

#[derive(Clone)]
pub struct ChainBridgePhala2Ethereum {
    evm_chainid: u8,
    rpc: String,
}

#[allow(dead_code)]
impl ChainBridgePhala2Ethereum {
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
impl BridgeExecutor for ChainBridgePhala2Ethereum {
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
