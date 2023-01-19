use crate::prelude::BridgeExecutor;
use crate::prelude::Error;
use crate::transactors::XtokenClient;
use alloc::vec::Vec;
use pink_web3::ethabi::Address;

use pink_subrpc::ExtraParam;
use pink_web3::{
    api::{Eth, Namespace},
    contract::Contract,
    keys::pink::KeyPair,
    transports::PinkHttp,
};

#[derive(Clone)]
pub struct Moonbeam2PhalaExecutor {
    bridge_contract: XtokenClient,
}

impl Moonbeam2PhalaExecutor {
    #[allow(dead_code)]
    pub fn new(rpc: &str, xtoken_address: Address) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let bridge_contract = XtokenClient {
            contract: Contract::from_json(
                eth,
                xtoken_address,
                include_bytes!("../../abis/xtokens-abi.json"),
            )
            .expect("Bad abi data"),
        };

        Self { bridge_contract }
    }
}

impl BridgeExecutor for Moonbeam2PhalaExecutor {
    fn transfer(
        &self,
        signer: [u8; 32],
        asset_contract_address: Vec<u8>,
        recipient: Vec<u8>,
        amount: u128,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let signer = KeyPair::from(signer);
        let token_address = Address::from_slice(&asset_contract_address);
        // TODO: better error handling
        let tx_id = self
            .bridge_contract
            .transfer(
                signer,
                token_address,
                amount,
                // parents = 1
                1,
                // parachain
                2035,
                // any
                0,
                recipient,
                extra.nonce,
            )
            .map_err(|_| Error::FailedToSubmitTransaction)?;
        // dbg!(tx_id);
        Ok(tx_id.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use crate::utils::ToArray;

    use super::*;
    use pink_subrpc::ExtraParam;
    use primitive_types::H160;

    #[test]
    #[ignore]
    fn moonbeam_xtokens() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let exec = Moonbeam2PhalaExecutor::new(
            "https://moonbeam.public.blastapi.io",
            H160::from_str("0x0000000000000000000000000000000000000804").unwrap(),
        );
        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();
        let recipient =
            hex::decode("da1ada496c0e6e3c122aa17f51ccd7254782effab31b24575d54e0350e7f2f6a")
                .unwrap();
        exec.transfer(
            signer,
            hex::decode("ffffffff63d24ecc8eb8a7b5d0803e900f7b6ced").unwrap(),
            recipient,
            1_000_000_000_000,
            ExtraParam::default(),
        )
        .unwrap();
        // test txn: https://moonbeam.moonscan.io/tx/0x47a5fdea2e3bb807296b7d7c5e708b4db5a0aca732ef37ee0e173df3d3942872
    }
}
