use pink_subrpc as subrpc;

use crate::traits::{common::Error, executor::BridgeExecutor};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use scale::Decode;
use sp_runtime::{traits::ConstU32, BoundedSlice};
use subrpc::{create_transaction, send_transaction, ExtraParam};
use xcm::v3::{AssetId, Fungibility, Junction, Junctions, MultiAsset, MultiLocation, Weight};

#[derive(Clone)]
pub struct ChainBridgePhala2Ethereum {
    evm_chainid: u8,
    rpc: String,
}

#[derive(Clone)]
pub struct SygmaBridgePhala2Ethereum {
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
impl SygmaBridgePhala2Ethereum {
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
                Junction::from(
                    BoundedSlice::<u8, ConstU32<32>>::try_from("cb".as_bytes())
                        .or(Err(Error::InvalidMultilocation))?,
                ),
                Junction::GeneralIndex(self.evm_chainid as u128),
                Junction::from(
                    BoundedSlice::<u8, ConstU32<32>>::try_from(&recipient[..])
                        .or(Err(Error::InvalidMultilocation))?,
                ),
            ),
        );
        let dest_weight: core::option::Option<u64> = Some(5_000_000_000u64);
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

#[allow(dead_code)]
impl BridgeExecutor for SygmaBridgePhala2Ethereum {
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
                Junction::from(
                    BoundedSlice::<u8, ConstU32<32>>::try_from("sygma".as_bytes())
                        .or(Err(Error::InvalidMultilocation))?,
                ),
                Junction::GeneralIndex(self.evm_chainid as u128),
                Junction::from(
                    BoundedSlice::<u8, ConstU32<32>>::try_from(&recipient[..])
                        .or(Err(Error::InvalidMultilocation))?,
                ),
            ),
        );
        let signed_tx = create_transaction(
            &signer,
            "phala",
            &self.rpc,
            0x52u8,
            0x0u8,
            (multi_asset, dest, None as Option<Weight>),
            extra,
        )
        .map_err(|_| Error::InvalidSignature)?;
        println!("{:?}", hex::encode(signed_tx.clone()));
        let tx_id =
            send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)?;
        Ok(tx_id)
    }
}

#[cfg(test)]
mod tests {
    use super::ChainBridgePhala2Ethereum;
    use crate::constants::{CHAINBRIDGE_ID_ETHEREUM, SYGMABRIDGE_ID_ETHEREUM};
    use crate::executors::bridge_executors::phala_to_ethereum::SygmaBridgePhala2Ethereum;
    use crate::traits::executor::BridgeExecutor;
    use crate::utils::ToArray;
    use dotenv::dotenv;
    use pink_subrpc::ExtraParam;
    use scale::Encode;
    use xcm::v3::{Junctions, MultiLocation};

    #[test]
    #[ignore]
    fn chainbridge_pha_phala2ethereum_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let chainbridge =
            ChainBridgePhala2Ethereum::new(CHAINBRIDGE_ID_ETHEREUM, "http://127.0.0.1:30444");
        let pha_location = MultiLocation::new(0, Junctions::Here);
        let recipient: Vec<u8> =
            hex_literal::hex!("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").into();

        let tx_id = chainbridge.transfer(
            signer,
            pha_location.encode(),
            recipient,
            // 301 PHA (decimals is 12 on Thala, 300 as fee, recipient will receive 1 PHA)
            301_000_000_000_000u128,
            ExtraParam::default(),
        );
        // https://goerli.etherscan.io/tx/0xb04f4370f88abfcd32523a201548061a73f94ae2b675fe7de096586a727b737e
        println!(
            "ChainBridgePhala2Ethereum: send tx {:?}",
            hex::encode(tx_id.unwrap())
        );
    }

    #[test]
    // #[ignore]
    fn sygmabridge_pha_phala2ethereum_works() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let sygmabridge = SygmaBridgePhala2Ethereum::new(
            SYGMABRIDGE_ID_ETHEREUM,
            "https://subbridge-test.phala.network/rhala/ws",
        );
        let pha_location = MultiLocation::new(0, Junctions::Here);
        let recipient: Vec<u8> =
            hex_literal::hex!("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").into();

        let tx_id = sygmabridge.transfer(
            signer,
            pha_location.encode(),
            recipient,
            // 1.01 PHA (decimals is 12 on Rhala, 0.1 as fee, recipient will receive 1 PHA)
            1_100_000_000_000u128,
            ExtraParam::default(),
        );
        println!(
            "SygmaBridgePhala2Ethereum: send tx {:?}",
            hex::encode(tx_id.unwrap())
        );
    }
}
