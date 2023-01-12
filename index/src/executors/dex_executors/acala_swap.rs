use pink_subrpc::{create_transaction, send_transaction};

use crate::constants::ACALA_PARACHAIN_ID;
use crate::prelude::DexExecutor;
use crate::traits::{common::Error, executor::BridgeExecutor};
use crate::utils::ToArray;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use scale::Decode;
use scale::{Compact, Encode};

use xcm::v1::{prelude::*, AssetId, Fungibility, Junctions, MultiAsset, MultiLocation};

#[derive(Clone)]
pub struct AcalaDexExecutor {
    rpc: String,
}

#[allow(dead_code)]
impl AcalaDexExecutor {
    pub fn new(rpc: &str) -> Self
    where
        Self: Sized,
    {
        Self {
            rpc: rpc.to_string(),
        }
    }
}

impl AcalaDexExecutor {
    fn aggregated_swap(
        &self,
        signer: [u8; 32],
        path: Vec<u8>,
        spend: u128,
        _recipient: Vec<u8>,
    ) -> core::result::Result<(), Error> {
        let amount_out = Compact(1_u8);
        let amount_in = Compact(spend);

        let mut path = path.as_ref();
        let path = Vec::<AggregatedSwapPath>::decode(&mut path).unwrap();
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
        )
        .map_err(|_| Error::InvalidSignature)?;
        let _tx_id =
            send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)?;
        dbg!(hex::encode(_tx_id));
        Ok(())
    }
}

impl DexExecutor for AcalaDexExecutor {
    fn swap(
        &self,
        signer: [u8; 32],
        // TODO: to determind the content of this parameter
        // I will assume it's the encoded version of CurrencyId(defined in acala source code)
        // eg. vec![0, 1] is the serialization of aca,
        // but then there will extra and meaningless overhead: decode first, then encode again
        asset0: Vec<u8>,
        asset1: Vec<u8>,
        spend: u128,
        _recipient: Vec<u8>,
    ) -> core::result::Result<(), Error> {
        let amount_out = Compact(1_u8);
        let amount_in = Compact(spend);
        let mut asset0 = asset0.as_ref();
        let mut asset1 = asset1.as_ref();
        let token0 = CurrencyId::decode(&mut asset0).map_err(|_| Error::FailedToScaleDecode)?;
        let token1 = CurrencyId::decode(&mut asset1).map_err(|_| Error::FailedToScaleDecode)?;
        let path = vec![token0, token1];

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
        )
        .map_err(|_| Error::InvalidSignature)?;
        let _tx_id =
            send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)?;
        dbg!(hex::encode(_tx_id));
        Ok(())
    }
}

// Copy from https://github.com/AcalaNetwork/Acala/blob/master/primitives/src/currency.rs ,
// with modification
//
//
// 0 - 127: Polkadot Ecosystem tokens
// 0 - 19: Acala & Polkadot native tokens
// 20 - 39: External tokens (e.g. bridged)
// 40 - 127: Polkadot parachain tokens
//
// 128 - 255: Kusama Ecosystem tokens
// 128 - 147: Karura & Kusama native tokens
// 148 - 167: External tokens (e.g. bridged)
// 168 - 255: Kusama parachain tokens
#[derive(Debug, Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum TokenSymbol {
    // 0 - 19: Acala & Polkadot native tokens
    ACA = 0,
    AUSD = 1,
    DOT = 2,
    LDOT = 3,
    TAP = 4,
    // 20 - 39: External tokens (e.g. bridged)
    RENBTC = 20,
    CASH = 21,
    // 40 - 127: Polkadot parachain tokens

    // 128 - 147: Karura & Kusama native tokens
    KAR = 128,
    KUSD = 129,
    KSM = 130,
    LKSM = 131,
    TAI = 132,
    // 148 - 167: External tokens (e.g. bridged)
    // 149: Reserved for renBTC
    // 150: Reserved for CASH
    // 168 - 255: Kusama parachain tokens
    BNC = 168,
    VSKSM = 169,
    PHA = 170,
    KINT = 171,
    KBTC = 172,
}

#[derive(Debug, Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CurrencyId {
    Token(TokenSymbol),
}

#[derive(Debug, Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum AggregatedSwapPath {
    Dex(Vec<CurrencyId>),
    Taiga(u32, u32, u32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale::Compact;
    use scale::Encode;

    #[test]
    fn compact_encoding() {
        // https://polkadot.js.org/apps/?rpc=wss%3A%2F%2Facala.polkawallet.io#/extrinsics
        // when you input 1 in the supplyAmount field whose type is `Compact<u128>`，
        // in the encoding detail the field is shown as `04`, which is exactly the same as
        // `Compact(1_u128).encode()`，
        // that is to say, we don't need to encode the values first before putting then into the text boxes.
        let amount = Compact(1_u128);
        dbg!(hex::encode(amount.encode()));
        assert_eq!(amount.encode(), vec![0x04_u8]);
        // an extrinsic that succeeded: https://acala.subscan.io/extrinsic/2690131-2
        // so, 1_000_000_000_000 actually means 1 ACA, ACA's decimals is 12
        // AUSD's decimals is 12,
        // the swap remove 1 + gas ACA from the test account, which previously had the balance of 2 ACA
    }

    #[test]
    fn acala_types_encoding() {
        let token_aca = CurrencyId::Token(TokenSymbol::ACA);
        let token_ausd = CurrencyId::Token(TokenSymbol::AUSD);
        assert_eq!(token_aca.encode(), vec![0, 0]);
        let encoded_aca = vec![0_u8, 0];
        let mut encoded_aca = encoded_aca.as_ref();
        let token_aca2 = CurrencyId::decode(&mut encoded_aca).unwrap();
        assert_eq!(token_aca, token_aca2);
        let path: Vec<CurrencyId> = vec![token_aca, token_ausd];
        assert_eq!(path.encode(), hex::decode("0800000001").unwrap());
        dbg!(hex::encode(path.encode()));
    }

    #[test]
    fn acala_swap_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let executor = AcalaDexExecutor::new("https://acala-rpc.dwellir.com");
        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();
        let recipient: Vec<u8> = vec![];

        let token_ldot = CurrencyId::Token(TokenSymbol::LDOT);
        let token_ausd = CurrencyId::Token(TokenSymbol::AUSD);

        let taiga_path = AggregatedSwapPath::Taiga(0, 0, 1);
        let dex_path = AggregatedSwapPath::Dex(vec![token_ldot, token_ausd]);

        let path = vec![taiga_path, dex_path];
        let path = path.encode();

        // 0.01 dot
        let spend = 100_000_000;
        // https://acala.subscan.io/extrinsic/0x14575ccbbddbef7189e9402317eb9ce1d84ee0d2ddd44cf9738071c07fbad793
        assert!(executor
            .aggregated_swap(signer, path, spend, recipient)
            .is_ok());
    }
}
