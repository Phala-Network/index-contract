use pink_subrpc::{create_transaction, send_transaction};

use crate::constants::ACALA_PARACHAIN_ID;
use crate::traits::{common::Error, executor::BridgeExecutor};
use crate::utils::ToArray;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use scale::Decode;

use xcm::v1::{prelude::*, AssetId, Fungibility, Junctions, MultiAsset, MultiLocation};

#[derive(Clone)]
pub struct Phala2AcalaExecutor {
    rpc: String,
}

#[allow(dead_code)]
impl Phala2AcalaExecutor {
    pub fn new(rpc: &str) -> Self
    where
        Self: Sized,
    {
        Self {
            rpc: rpc.to_string(),
        }
    }
}

impl BridgeExecutor for Phala2AcalaExecutor {
    fn transfer(
        &self,
        signer: [u8; 32],
        asset: Vec<u8>,
        recipient: Vec<u8>,
        amount: u128,
    ) -> core::result::Result<(), Error> {
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
                Parachain(ACALA_PARACHAIN_ID),
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
        )
        .map_err(|_| Error::InvalidSignature)?;
        let _tx_id = send_transaction(&self.rpc, &signed_tx).map_err(|err| {
            dbg!(err);
            Error::SubRPCRequestFailed
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::constants::PHALA_PARACHAIN_ID;

    use super::*;
    use scale::Encode;

    #[test]
    #[ignore = "to prevent the private keys being leaked, run this test with `SECRET_KEY=<your-private-key> cargo test moonbeam_xtokens -- --nocapture`"]
    fn phala_to_acala() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let exec = Phala2AcalaExecutor::new("https://api.phala.network/rpc");
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
        // example: https://phala.subscan.io/extrinsic/1620712-2
        // note that network problems can cause Error::InvalidSignature, no idea why
        exec.transfer(signer, asset, recipient, 1_000_000_000_000)
            .unwrap();
    }
}
