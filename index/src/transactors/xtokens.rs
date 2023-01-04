use crate::prelude::Error;
use pink_web3::contract::Options;
use pink_web3::ethabi::{Address, Token};
use pink_web3::signing::Key;
use pink_web3::transports::resolve_ready;
use pink_web3::{contract::Contract, keys::pink::KeyPair, transports::PinkHttp};
use primitive_types::{H256, U256};

#[derive(Clone)]
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
        recipient: Vec<u8>,
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
                        let mut network_vec = network.to_be_bytes().to_vec();
                        let mut recipient = recipient;
                        bytes.append(&mut enum_id);
                        bytes.append(&mut recipient);
                        bytes.append(&mut network_vec);
                        bytes
                    },
                ),
            ]),
        ]);
        let amount: U256 = amount.into();
        let params = (token_address, amount, location, weight);

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
        dbg!(tx_id);
        Ok(tx_id)
    }
}
