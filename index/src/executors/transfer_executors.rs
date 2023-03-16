use pink_extension::AccountId;
use pink_subrpc::{create_transaction, send_transaction, ExtraParam};
use xcm::v1::prelude::*;

use crate::{
    assets::{AcalaAssetMap, CurrencyId, TokenType as AcalaTokenType},
    prelude::Error,
    utils::ToArray,
};
use scale::Decode;

type MultiAddress = sp_runtime::MultiAddress<AccountId, u32>;

#[derive(Clone)]
pub struct AcalaTransferExecutor {
    rpc: String,
}

enum AcalaAsset {
    Utility,
    Other(CurrencyId),
}

impl AcalaTransferExecutor {
    pub fn new() {}

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
        let account = MultiAddress::Address32(bytes);
        let asset_attrs = AcalaAssetMap::get_asset_attrs(&asset_location).ok_or(Error::BadAsset)?;
        let currency_id = CurrencyId::Token(asset_attrs.0);
        let asset_type = asset_attrs.1;
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
