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
pub struct MoonbeamXTokenExecutor {
    bridge_contract: XtokenClient,
    dest_chain_id: u32,
}

impl MoonbeamXTokenExecutor {
    #[allow(dead_code)]
    pub fn new(rpc: &str, xtoken_address: Address, dest_chain_id: u32) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let bridge_contract = XtokenClient {
            contract: Contract::from_json(
                eth,
                xtoken_address,
                include_bytes!("../../abis/xtokens-abi.json"),
            )
            .expect("Bad abi data"),
        };

        Self {
            bridge_contract,
            dest_chain_id,
        }
    }
}

impl BridgeExecutor for MoonbeamXTokenExecutor {
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
        let tx_id = self
            .bridge_contract
            .transfer(
                signer,
                token_address,
                amount,
                // parents = 1
                1,
                // parachain
                self.dest_chain_id,
                // any
                0,
                recipient,
                extra.nonce,
            )
            .map_err(|_| Error::FailedToSubmitTransaction)?;

        Ok(tx_id.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use crate::{
        prelude::{ACALA_PARACHAIN_ID, PHALA_PARACHAIN_ID},
        utils::ToArray,
    };

    use super::*;
    use pink_subrpc::ExtraParam;
    use primitive_types::H160;

    #[test]
    #[ignore]
    fn moonbeam_to_phala() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let exec = MoonbeamXTokenExecutor::new(
            "https://moonbeam.public.blastapi.io",
            H160::from_str("0x0000000000000000000000000000000000000804").unwrap(),
            PHALA_PARACHAIN_ID,
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

    #[test]
    #[ignore]
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
            ExtraParam::default(),
        )
        .unwrap();
        // test txn:
        // - https://moonbeam.subscan.io/xcm_message/polkadot-16d178dd10eb67113379520279b7cd5a8547999a
    }
}
