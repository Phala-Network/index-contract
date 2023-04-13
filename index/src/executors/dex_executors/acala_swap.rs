use pink_extension::ResultExt;
use pink_subrpc::{create_transaction, send_transaction, ExtraParam};
use xcm::v1::prelude::*;

use crate::assets::{AcalaAssetMap, AggregatedSwapPath, CurrencyId, TokenSymbol};

use crate::prelude::DexExecutor;
use crate::traits::common::Error;
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use scale::{Compact, Decode};

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
    #[allow(dead_code)]
    fn aggregated_swap(
        &self,
        signer: [u8; 32],
        path: Vec<u8>,
        spend: u128,
        _recipient: Vec<u8>,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let amount_out = Compact(1_u8);
        let amount_in = Compact(spend);

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
        let tx_id =
            send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)?;
        Ok(tx_id)
    }
}

impl DexExecutor for AcalaDexExecutor {
    fn swap(
        &self,
        signer: [u8; 32],
        // Encoded asset location
        asset0: Vec<u8>,
        // Encoded asset location
        asset1: Vec<u8>,
        spend: u128,
        _recipient: Vec<u8>,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let amount_out = Compact(1_u8);
        let amount_in = Compact(spend);

        let asset0_location: MultiLocation = Decode::decode(&mut asset0.as_slice())
            .log_err(&format!(
                "AcalaDexExecutor: FailedToScaleDecode, asset: {:?}",
                &asset0
            ))
            .map_err(|_| Error::FailedToScaleDecode)?;
        let asset1_location: MultiLocation = Decode::decode(&mut asset1.as_slice())
            .log_err(&format!(
                "AcalaDexExecutor: FailedToScaleDecode, asset: {:?}",
                &asset1
            ))
            .map_err(|_| Error::FailedToScaleDecode)?;

        let token0 =
            AcalaAssetMap::get_currency_id(&asset0_location).ok_or(Error::AssetNotRecognized)?;
        let token1 =
            AcalaAssetMap::get_currency_id(&asset1_location).ok_or(Error::AssetNotRecognized)?;

        // FIXME: hardcode for demo
        if token0 != CurrencyId::Token(TokenSymbol::DOT)
            || token1 != CurrencyId::Token(TokenSymbol::ACA)
        {
            pink_extension::debug!("AcalaDexExecutor: Unsupported trading pair",);
            return Err(Error::Unimplemented);
        }

        let taiga_path = AggregatedSwapPath::Taiga(0, 0, 1);
        // FIXME: Looks like first node is LDOT, represents dex will spend DOT
        let dex_path = AggregatedSwapPath::Dex(vec![
            CurrencyId::Token(TokenSymbol::LDOT),
            CurrencyId::Token(TokenSymbol::AUSD),
            token1,
        ]);
        let path = vec![taiga_path, dex_path];

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
        let tx_id =
            send_transaction(&self.rpc, &signed_tx).map_err(|_| Error::SubRPCRequestFailed)?;

        Ok(tx_id)
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::*;
    use crate::assets::{CurrencyId, TokenSymbol};
    use crate::utils::ToArray;
    use pink_subrpc::ExtraParam;
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
    #[ignore]
    fn acala_swap_dot_2_aca_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let encoded_dot_location = hex::decode("010200411f06080002").unwrap();
        let encoded_aca_location = hex::decode("010200411f06080000").unwrap();

        let executor = AcalaDexExecutor::new("https://acala-rpc.dwellir.com");
        // 0.005 DOT
        let spend = 50_000_000;
        let tx_id = executor
            .swap(
                signer,
                encoded_dot_location,
                encoded_aca_location,
                spend,
                vec![],
                ExtraParam::default(),
            )
            .unwrap();
        println!("AcalaDexExecutor swap transaction submitted: {:?}", &tx_id);
    }

    #[test]
    #[ignore]
    fn acala_aggregated_swap_works() {
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
            .aggregated_swap(signer, path, spend, recipient, ExtraParam::default())
            .is_ok());
    }

    use pink_web3::types::H160;
    #[test]
    fn abcde() {
        // 307833613632613439383062393532433932663464343234336334413030393333364565306132366542 => 0x3a62a4980b952C92f4d4243c4A009336Ee0a26eB
        let a = hex::decode("307833613632613439383062393532433932663464343234336334413030393333364565306132366542").unwrap();
        let s = String::from_utf8_lossy(&a);
        let b = H160::from_str(&s).unwrap();
        dbg!(s);
        dbg!(b);
    }
}
