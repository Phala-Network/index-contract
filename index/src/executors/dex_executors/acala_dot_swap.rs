use pink_subrpc::{create_transaction, send_transaction, ExtraParam};

use crate::assets::{AggregatedSwapPath, CurrencyId, TokenSymbol};
use crate::prelude::DexExecutor;
use crate::traits::common::Error;
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use scale::{Compact, Decode, Encode};

#[derive(Clone)]
pub struct AcalaDotSwapExecutor {
    rpc: String,
}

#[allow(dead_code)]
impl AcalaDotSwapExecutor {
    pub fn new(rpc: &str) -> Self
    where
        Self: Sized,
    {
        Self {
            rpc: rpc.to_string(),
        }
    }
}

impl AcalaDotSwapExecutor {
    fn aggregated_swap(
        &self,
        signer: [u8; 32],
        path: Vec<u8>,
        spend: u128,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let amount_out = Compact(1_u8);
        let amount_in = Compact(spend);

        pink_extension::debug!("Start to create swap transaction");
        let mut path = path.as_ref();
        let path =
            Vec::<AggregatedSwapPath>::decode(&mut path).map_err(|_| Error::FailedToScaleDecode)?;
        let signed_tx = create_transaction(
            &signer,
            "acala",
            &self.rpc,
            // the call index of acala dex module
            //0x5b_u8,
            // the call index of acala aggregateddex module
            0x5d_u8,
            0x0u8,
            (path, amount_in, amount_out),
            extra,
        )
        .map_err(|_| Error::InvalidSignature)?;
        pink_extension::debug!("Create swap signed transaction: {:?}", &signed_tx);

        let tx_id =
            send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)?;
        pink_extension::debug!("Swap transaction submitted: {:?}", hex::encode(&tx_id));

        Ok(tx_id)
    }
}

impl DexExecutor for AcalaDotSwapExecutor {
    fn swap(
        &self,
        signer: [u8; 32],
        _asset0: Vec<u8>,
        _asset1: Vec<u8>,
        spend: u128,
        _recipient: Vec<u8>,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let token_ldot = CurrencyId::Token(TokenSymbol::LDOT);
        let token_ausd = CurrencyId::Token(TokenSymbol::AUSD);

        let taiga_path = AggregatedSwapPath::Taiga(0, 0, 1);
        let dex_path = AggregatedSwapPath::Dex(vec![token_ldot, token_ausd]);

        let path = vec![taiga_path, dex_path];
        let path = path.encode();

        let tx_id = self.aggregated_swap(signer, path, spend, extra)?;
        Ok(tx_id)
    }
}
