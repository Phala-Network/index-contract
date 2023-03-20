use pink_extension::AccountId;
use pink_subrpc::{create_transaction, send_transaction, ExtraParam};
use xcm::v1::prelude::*;

use crate::{
    assets::{AcalaAssetMap, CurrencyId, TokenType as AcalaTokenType},
    prelude::Error,
    utils::ToArray,
};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::result::Result;
use scale::{Compact, Decode};

type MultiAddress = sp_runtime::MultiAddress<AccountId, u32>;

#[derive(Clone)]
pub struct AcalaTransferExecutor {
    rpc: String,
}

impl AcalaTransferExecutor {
    pub fn new(rpc: &str) -> Self
    where
        Self: Sized,
    {
        Self {
            rpc: rpc.to_string(),
        }
    }

    fn transfer(
        &self,
        signer: [u8; 32],
        asset: Vec<u8>,
        recipient: Vec<u8>,
        amount: u128,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let asset_location =
            MultiLocation::decode(&mut asset.as_slice()).map_err(|_| Error::FailedToScaleDecode)?;
        let bytes: [u8; 32] = recipient.to_array();
        let recipient = MultiAddress::Address32(bytes);
        let asset_attrs = AcalaAssetMap::get_asset_attrs(&asset_location).ok_or(Error::BadAsset)?;
        let currency_id = CurrencyId::Token(asset_attrs.0);
        let asset_type = asset_attrs.1;
        let amount = Compact(amount);

        match asset_type {
            AcalaTokenType::Utility => {
                let signed_tx = create_transaction(
                    &signer,
                    "acala",
                    &self.rpc,
                    // Balance
                    0x0au8,
                    // Transfer
                    00,
                    (recipient, amount),
                    extra,
                )
                .map_err(|_| Error::InvalidSignature)?;

                send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)
            }
            _ => {
                let currency_id = match asset_type {
                    AcalaTokenType::Foreign => {
                        let foreign_asset_id = asset_attrs.2.ok_or(Error::BadAsset)?;
                        CurrencyId::ForeignAsset(foreign_asset_id)
                    }
                    _ => currency_id,
                };

                let signed_tx = create_transaction(
                    &signer,
                    "acala",
                    &self.rpc,
                    // Currencies
                    0x0cu8,
                    // Transfer
                    00,
                    (recipient, currency_id, amount),
                    extra,
                )
                .map_err(|_| Error::InvalidSignature)?;

                send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{CurrencyId, TokenSymbol};
    use crate::utils::ToArray;
    use pink_subrpc::ExtraParam;
    use scale::Compact;
    use scale::Encode;

    #[test]
    fn acala_transfer_encoding_is_right() {
        let lc_pha: MultiLocation = MultiLocation::new(1, X1(Parachain(2004)));
        let bytes = hex::decode("cee6b60451fe18916873a0775b8ab8535843b90b1d92ccc1b75925c375790623")
            .unwrap()
            .to_array();
        let recipient = MultiAddress::Address32(bytes);
        let asset_attrs = AcalaAssetMap::get_asset_attrs(&lc_pha)
            .ok_or(Error::BadAsset)
            .unwrap();
        let currency_id = CurrencyId::Token(asset_attrs.0);
        let asset_type = asset_attrs.1;
        let foreign_asset_id = asset_attrs.2.unwrap();
        // 00 aa / 010900
        let currency_id = CurrencyId::ForeignAsset(foreign_asset_id);
        let amount = Compact(1000000000000_u128);
        let call_data = (recipient, currency_id, amount).encode();
        dbg!(hex::encode(call_data));
    }

    #[test]
    fn acala_transfer_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let executor = AcalaTransferExecutor::new("https://acala-rpc.dwellir.com");
    }
}
