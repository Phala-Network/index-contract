use crate::prelude::BridgeExecutor;
use crate::prelude::Error;
use crate::transactors::XtokenClient;
use alloc::vec::Vec;
use pink_web3::ethabi::Address;

use pink_web3::{
    api::{Eth, Namespace},
    contract::Contract,
    keys::pink::KeyPair,
    transports::PinkHttp,
};

#[derive(Clone)]
pub struct Moonbeam2AcalaExecutor {
    bridge_contract: XtokenClient,
}

impl Moonbeam2AcalaExecutor {
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

impl BridgeExecutor for Moonbeam2AcalaExecutor {
    fn transfer(
        &self,
        signer: [u8; 32],
        asset_contract_address: Vec<u8>,
        recipient: Vec<u8>,
        amount: u128,
    ) -> core::result::Result<(), Error> {
        let signer = KeyPair::from(signer);
        let token_address = Address::from_slice(&asset_contract_address);
        // TODO: better error handling
        _ = self
            .bridge_contract
            .transfer(
                signer,
                token_address,
                amount,
                // parents = 1
                1,
                // parachain
                2000,
                // any
                0,
                recipient,
            )
            .unwrap();
        // dbg!(tx_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use crate::utils::ToArray;

    use super::*;
    use primitive_types::H160;

    #[test]
    fn moonbeam_to_acala_xcdot() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let exec = Moonbeam2AcalaExecutor::new(
            "https://moonbeam.public.blastapi.io",
            H160::from_str("0x0000000000000000000000000000000000000804").unwrap(),
        );
        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();
        let recipient =
            hex::decode("cee6b60451fe18916873a0775b8ab8535843b90b1d92ccc1b75925c375790623")
                .unwrap();
        exec.transfer(
            signer,
            // https://moonbeam.moonscan.io/token/0xffffffff1fcacbd218edc0eba20fc2308c778080
            hex::decode("FfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080").unwrap(),
            recipient,
            // 0.1 xcdot
            // too small an amount will cause transaction failure: https://moonbeam.subscan.io/xcm_message/polkadot-270774e1bdb5eb294b2e04bb62b1e2c0d639dcf7
            // polkadot said too expensive
            1_000_000_000,
        )
        .unwrap();
        // test txn:
        // - https://moonbeam.subscan.io/xcm_message/polkadot-16d178dd10eb67113379520279b7cd5a8547999a
    }
}
