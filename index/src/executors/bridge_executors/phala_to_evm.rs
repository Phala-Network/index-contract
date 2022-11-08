use crate::{
    prelude::{Address, Executor},
    subrpc::{create_transaction, send_transaction, UnsignedExtrinsic},
    traits::Error,
};
use scale::Encode;
use xcm::v1::MultiAsset;
use xcm::v1::{AssetId, Fungibility, Junction, Junctions, MultiLocation};
use hex_literal::hex;

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
        recipient: Address,
    ) -> core::result::Result<(), Error> {
        match recipient {
            Address::EthAddr(addr) => {
                let addr = addr.to_fixed_bytes().to_vec();
                let multi_asset = MultiAsset {
                    id: AssetId::Concrete(Junctions::Here.into()),
                    fun: Fungibility::Fungible(amount as u128),
                };

                let dest = MultiLocation::new(
                    0,
                    Junctions::X3(
                        Junction::GeneralKey(b"cb".to_vec().try_into().unwrap()),
                        Junction::GeneralIndex(0u128),
                        Junction::GeneralKey(addr.try_into().unwrap()),
                    ),
                );

                let destWeight: std::option::Option<u64> = None;

                let call_data = UnsignedExtrinsic {
                    pallet_id: 0x52u8,
                    call_id: 0x0u8,
                    call: (multi_asset, dest, destWeight),
                };

                let mut bytes = Vec::new();
                call_data.encode_to(&mut bytes);
                let expected: Vec<u8> = hex!("5200000000000f00d01306c21101000306086362050006508266b3183ccc58f3d145d7a4894547bd55d7739700").into();
                assert_eq!(bytes, expected);

                let signed_tx = create_transaction(&signer, "phala", &self.rpc, call_data)?;
                let tx_id = send_transaction(&self.rpc, &signed_tx)?;

                Ok(())
            }
            _ => Err(Error::InvalidAddress),
        }
    }
}
