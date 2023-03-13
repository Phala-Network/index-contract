use pink_subrpc::{create_transaction, send_transaction, ExtraParam};

use crate::traits::{common::Error, executor::BridgeExecutor};
use crate::utils::ToArray;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use scale::Decode;

use xcm::v1::{prelude::*, AssetId, Fungibility, Junctions, MultiAsset, MultiLocation};

#[derive(Clone)]
pub struct PhalaXTransferExecutor {
    rpc: String,
    dest_chain_id: u32,
}

#[allow(dead_code)]
impl PhalaXTransferExecutor {
    pub fn new(rpc: &str, dest_chain_id: u32) -> Self
    where
        Self: Sized,
    {
        Self {
            rpc: rpc.to_string(),
            dest_chain_id,
        }
    }
}

impl BridgeExecutor for PhalaXTransferExecutor {
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
        let recipient_32: [u8; 32] = recipient.to_array();
        let dest = MultiLocation::new(
            1,
            Junctions::X2(
                Parachain(self.dest_chain_id),
                AccountId32 {
                    network: NetworkId::Any,
                    id: recipient_32,
                },
            ),
        );
        let dest_weight: core::option::Option<u64> = Some(6000000000);
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
    use crate::{constants::PHALA_PARACHAIN_ID, prelude::MOONBEAM_PARACHAIN_ID};

    use super::*;
    use crate::constants::ACALA_PARACHAIN_ID;
    use pink_subrpc::ExtraParam;
    use scale::Encode;

    #[test]
    #[ignore]
    fn phala_to_acala() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let exec = PhalaXTransferExecutor::new("https://api.phala.network/rpc", ACALA_PARACHAIN_ID);
        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();
        let recipient =
            hex::decode("663be7a0bda61c0a6eaa2f15a58f02f5cec9e72a23911230a2894a117b9d981a")
                .unwrap();
        let asset = MultiLocation {
            parents: 1,
            interior: Junctions::X1(Parachain(PHALA_PARACHAIN_ID)),
        };
        let asset = asset.encode();

        exec.transfer(
            signer,
            asset,
            recipient,
            1_000_000_000_000,
            ExtraParam::default(),
        )
        .unwrap();
    }

    #[test]
    #[ignore]
    fn phala_to_moonbeam() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let exec =
            PhalaXTransferExecutor::new("https://api.phala.network/rpc", MOONBEAM_PARACHAIN_ID);
        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();
        let recipient =
            hex::decode("663be7a0bda61c0a6eaa2f15a58f02f5cec9e72a23911230a2894a117b9d981a")
                .unwrap();
        let asset = MultiLocation {
            parents: 1,
            interior: Junctions::X1(Parachain(PHALA_PARACHAIN_ID)),
        };
        let asset = asset.encode();

        exec.transfer(
            signer,
            asset,
            recipient,
            1_000_000_000_000,
            ExtraParam::default(),
        )
        .unwrap();
    }
}
