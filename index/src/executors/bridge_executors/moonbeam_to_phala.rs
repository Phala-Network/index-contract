// use dyn_clone::DynClone;

use crate::{prelude::Error, transactors::ChainBridgeClient, utils::ToArray};
use pink_web3::contract::tokens::{Tokenizable, Tokenize};
use pink_web3::contract::Options;
use pink_web3::ethabi::{Address, Bytes, Token};
use pink_web3::signing::Key;
use pink_web3::transports::resolve_ready;
use pink_web3::{
    api::{Eth, Namespace},
    contract::Contract,
    keys::pink::KeyPair,
    transports::PinkHttp,
};
use primitive_types::{H160, H256, U256};
use scale::Encode;
use xcm::v0::NetworkId;
use xcm::v1::{Junction, Junctions, MultiLocation};
use xcm::v2::{Weight, WeightLimit};

pub trait BridgeExecutor /* : DynClone */ {
    fn transfer(
        &self,
        signer: [u8; 32],
        recipient: Vec<u8>,
        amount: u128,
    ) -> core::result::Result<H256, Error>;
}

pub struct XtokenClient {
    pub contract: Contract<PinkHttp>,
}

impl XtokenClient {
    pub fn transfer(
        &self,
        signer: KeyPair,
        token_address: Address,
        amount: u128,
        parents: u8,
        parachain: u32,
        network: u8,
        recipient: [u8; 32],
    ) -> core::result::Result<H256, Error> {
        let weight: u64 = 6000000000;
        let location = Token::Tuple(vec![
            Token::Uint(parents.into()),
            Token::Array(vec![
                Token::Bytes(
                    // Parachain(#[codec(compact)] u32),
                    {
                        let mut bytes: Vec<u8> = vec![];
                        let mut enum_id = (0 as u8).to_be_bytes().to_vec();
                        let mut chain_id = parachain.to_be_bytes().to_vec();
                        bytes.append(&mut enum_id);
                        bytes.append(&mut chain_id);
                        bytes
                    },
                ),
                Token::Bytes(
                    // AccountId32 { network: NetworkId, id: [u8; 32] },
                    {
                        let mut bytes: Vec<u8> = vec![];
                        let mut enum_id = (1 as u8).to_be_bytes().to_vec();
                        let mut recipient_vec = recipient.to_vec();
                        let mut network_vec = network.to_be_bytes().to_vec();
                        bytes.append(&mut enum_id);
                        bytes.append(&mut recipient_vec);
                        bytes.append(&mut network_vec);
                        bytes
                    },
                ),
            ]),
        ]);
        let amount: U256 = amount.into();
        let params = (token_address, amount, location, weight);

        dbg!(signer.address());
        // Estiamte gas before submission
        let gas = resolve_ready(self.contract.estimate_gas(
            "transfer",
            params.clone(),
            signer.address(),
            Options::default(),
        ))
        .expect("FIXME: failed to estiamte gas");

        dbg!(&gas);

        // Actually submit the tx (no guarantee for success)
        let tx_id = resolve_ready(self.contract.signed_call(
            "transfer",
            params,
            Options::with(|opt| opt.gas = Some(gas)),
            signer,
        ))
        .expect("FIXME: submit failed");
        Ok(tx_id)
    }
}

pub struct Moonbeam2PhalaExecutor {
    // (asset_contract_address, resource_id)
    //assets: (Address, [u8; 32]),
    asset_contract_address: Address,
    bridge_contract: XtokenClient,
}

impl Moonbeam2PhalaExecutor {
    pub fn new(rpc: &str, bridge_address: Address, asset_contract_address: Address) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let bridge_contract = XtokenClient {
            contract: Contract::from_json(
                eth,
                bridge_address,
                include_bytes!("../../abis/xtokens-abi.json"),
            )
            .expect("Bad abi data"),
        };

        Self {
            asset_contract_address,
            bridge_contract,
        }
    }
}

impl BridgeExecutor for Moonbeam2PhalaExecutor {
    fn transfer(
        &self,
        signer: [u8; 32],
        recipient: Vec<u8>,
        amount: u128,
    ) -> core::result::Result<H256, Error> {
        let signer = KeyPair::from(signer);
        let recipient_32: [u8; 32] = recipient.to_array();
        let interior = Junctions::X2(
            Junction::Parachain(2035),
            Junction::AccountId32 {
                network: NetworkId::Any,
                id: recipient_32,
            },
        );
        dbg!(hex::encode(Junction::Parachain(2035).encode()));
        let tx_id = self
            .bridge_contract
            .transfer(
                signer,
                self.asset_contract_address.clone(),
                amount,
                1,
                2035,
                0,
                recipient_32,
            )
            .unwrap();
        Ok(tx_id)
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::*;
    use primitive_types::H160;

    #[test]
    fn moonbeam_xtokens() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let exec = Moonbeam2PhalaExecutor::new(
            "https://moonbeam.public.blastapi.io",
            H160::from_str("0x0000000000000000000000000000000000000804").unwrap(),
            H160::from_str("0xffffffff63d24ecc8eb8a7b5d0803e900f7b6ced").unwrap(),
        );
        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();
        let recipient =
            hex::decode("da1ada496c0e6e3c122aa17f51ccd7254782effab31b24575d54e0350e7f2f6a")
                .unwrap();
        let tx_id = exec.transfer(signer, recipient, 1_000_000_000_000).unwrap();
        dbg!(tx_id);
    }
}
