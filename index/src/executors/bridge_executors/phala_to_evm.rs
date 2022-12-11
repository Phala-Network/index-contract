use crate::{
    prelude::Executor,
    traits::common::Error,
    traits::common::{Address, Amount},
};
use alloc::string::{String, ToString};
use xcm::v1::{AssetId, Fungibility, Junction, Junctions, MultiAsset, MultiLocation};
use subrpc::{create_transaction, send_transaction};

pub struct Phala2EvmExecutor {
    rpc: String,
}

impl Executor for Phala2EvmExecutor {
    fn new(
        _bridge_address: Address,
        _abi_json: &[u8],
        rpc: &str,
    ) -> core::result::Result<Self, Error>
    where
        Self: Sized,
    {
        Ok(Self {
            rpc: rpc.to_string(),
        })
    }

    fn transfer(
        &self,
        signer: [u8; 32],
        _token_rid: primitive_types::H256, // TODO: this param is useless,  that means the interface need to be changed
        amount: Amount,
        recipient: Address,
    ) -> core::result::Result<(), Error> {
        match recipient {
            Address::EthAddr(addr) => match amount {
                Amount::U128(amount) => {
                    let addr = addr.to_fixed_bytes().to_vec();
                    let multi_asset = MultiAsset {
                        id: AssetId::Concrete(Junctions::Here.into()),
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
                            Junction::GeneralIndex(0u128),
                            Junction::GeneralKey(
                                addr.try_into().or(Err(Error::InvalidMultilocation))?,
                            ),
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
                    ).map_err(|_| Error::InvalidSignature)?;
                    let _tx_id = send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)?;
                    Ok(())
                }
                _ => Err(Error::InvalidAmount),
            },
            _ => Err(Error::InvalidAddress),
        }
    }
}
