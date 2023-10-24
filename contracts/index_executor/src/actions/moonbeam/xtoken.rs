use alloc::{vec, vec::Vec};
use pink_web3::{
    api::{Eth, Namespace},
    contract::{tokens::Tokenize, Contract},
    ethabi::{Address, Token},
    transports::PinkHttp,
    types::U256,
};

use crate::call::{Call, CallBuilder, CallParams, EvmCall};
use crate::step::Step;

#[derive(Clone)]
pub enum XTokenDestChain {
    Relaychain,
    Parachain(u32),
}

#[derive(Clone)]
pub struct XTokenBridge {
    _eth: Eth<PinkHttp>,
    xtoken: Contract<PinkHttp>,
    dest_chain: XTokenDestChain,
}

impl XTokenBridge {
    pub fn new(rpc: &str, xtoken_address: Address, dest_chain: XTokenDestChain) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let xtoken = Contract::from_json(
            eth.clone(),
            xtoken_address,
            include_bytes!("../../abi/xtokens-abi.json"),
        )
        .expect("Bad abi data");

        Self {
            _eth: eth,
            xtoken,
            dest_chain,
        }
    }
}

impl CallBuilder for XTokenBridge {
    fn build_call(&self, step: Step) -> Result<Call, &'static str> {
        let spend_asset = Address::from_slice(&step.spend_asset);
        // We don't use it
        let receive_asset = Address::from_slice(&[0; 20]);
        let mut recipient = step.recipient.ok_or("MissingRecipient")?;
        let spend_amount = U256::from(step.spend_amount.ok_or("MissingSpendAmount")?);

        let weight: u64 = 6000000000;
        let location = match self.dest_chain {
            XTokenDestChain::Relaychain => {
                Token::Tuple(vec![
                    Token::Uint(1_u8.into()),
                    Token::Array(vec![Token::Bytes(
                        // AccountId32 { network: NetworkId, id: [u8; 32] },
                        {
                            let mut bytes: Vec<u8> = vec![];
                            let mut enum_id = 1_u8.to_be_bytes().to_vec();
                            let mut network_vec = 0_u8.to_be_bytes().to_vec();
                            bytes.append(&mut enum_id);
                            bytes.append(&mut recipient);
                            bytes.append(&mut network_vec);
                            bytes
                        },
                    )]),
                ])
            }
            XTokenDestChain::Parachain(parachain_id) => {
                Token::Tuple(vec![
                    Token::Uint(1_u8.into()),
                    Token::Array(vec![
                        Token::Bytes(
                            // Parachain(#[codec(compact)] u32),
                            {
                                let mut bytes: Vec<u8> = vec![];
                                let mut enum_id = 0_u8.to_be_bytes().to_vec();
                                let mut chain_id = parachain_id.to_be_bytes().to_vec();
                                bytes.append(&mut enum_id);
                                bytes.append(&mut chain_id);
                                bytes
                            },
                        ),
                        Token::Bytes(
                            // AccountId32 { network: NetworkId, id: [u8; 32] },
                            {
                                let mut bytes: Vec<u8> = vec![];
                                let mut enum_id = 1_u8.to_be_bytes().to_vec();
                                let mut network_vec = 0_u8.to_be_bytes().to_vec();
                                bytes.append(&mut enum_id);
                                bytes.append(&mut recipient);
                                bytes.append(&mut network_vec);
                                bytes
                            },
                        ),
                    ]),
                ])
            }
        };
        let bridge_params = (spend_asset, spend_amount, location, weight);
        let bridge_func = self
            .xtoken
            .abi()
            .function("transfer")
            .map_err(|_| "NoFunctionFound")?;
        let bridge_calldata = bridge_func
            .encode_input(&bridge_params.into_tokens())
            .map_err(|_| "EncodeParamError")?;

        Ok(Call {
            params: CallParams::Evm(EvmCall {
                target: self.xtoken.address(),
                calldata: bridge_calldata,
                value: U256::from(0),

                // Bridge operation do not need do settlement on source chain, because it must be the
                // last step on source chain
                need_settle: false,
                update_offset: U256::from(36),
                update_len: U256::from(32),
                spender: self.xtoken.address(),
                spend_asset,
                spend_amount,
                receive_asset,
            }),
            input_call: None,
            call_index: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::ToArray;

    use super::*;
    use pink_web3::contract::Options;
    use pink_web3::keys::pink::KeyPair;
    use pink_web3::transports::{resolve_ready, PinkHttp};
    use pink_web3::types::H160;

    #[test]
    #[ignore]
    fn test_transfer_dot_to_polkadot() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let rpc = "https://moonbeam.api.onfinality.io/public";
        // Handler on Moonbeam
        let handler_address: H160 =
            H160::from_slice(&hex::decode("B8D20dfb8c3006AA17579887ABF719DA8bDf005B").unwrap());
        let transport = Eth::new(PinkHttp::new(rpc.clone()));
        let handler = Contract::from_json(
            transport,
            handler_address,
            include_bytes!("../../abi/handler.json"),
        )
        .unwrap();
        let moonbeam_xtoken: [u8; 20] =
            hex_literal::hex!("0000000000000000000000000000000000000804");
        let xtoken = XTokenBridge::new(&rpc, moonbeam_xtoken.into(), XTokenDestChain::Relaychain);

        let mut call = xtoken
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Moonbeam"),
                dest_chain: String::from("Polkadot"),
                // xcDOT
                spend_asset: hex::decode("FfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080").unwrap(),
                // MuliLocation: (0, Here)
                receive_asset: hex::decode("0000").unwrap(),
                sender: Some(hex::decode("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").unwrap()),
                recipient: Some(
                    hex::decode("7804e66ec9eea3d8daf6273ffbe0a8af25a8879cf43f14d0ebbb30941f578242")
                        .unwrap(),
                ),
                // 0.05 xcDOT
                spend_amount: Some(500_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        // Apply index mannually
        call.input_call = Some(0);
        call.call_index = Some(0);

        // Make sure handler address hold enough spend asset and native asset (e.g. ETH).
        // Because handler is the account who spend and pay fee on behalf

        // Estiamte gas before submission
        let gas = resolve_ready(handler.estimate_gas(
            "batchCall",
            vec![call.clone()],
            // Worker address
            Address::from_slice(&hex::decode("bf526928373748b00763875448ee905367d97f96").unwrap()),
            Options::default(),
        ))
        .map_err(|e| {
            println!("Failed to estimated step gas cost with error: {:?}", e);
            "FailedToEstimateGas"
        })
        .unwrap();

        // Tested on Moonbeam: https://moonscan.io/tx/0xe3c3e3e41cc742575f3b5dc75d8954c427e430ac63c022bc8af46ed544f0782e
        // Received on Polkadot: https://polkadot.subscan.io/xcm_message/polkadot-db80cb5e14a2f2be83caffddd5c5267a8bef0b1a
        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let _tx_id: primitive_types::H256 = resolve_ready(handler.signed_call(
            "batchCall",
            vec![call],
            Options::with(|opt| opt.gas = Some(gas * 15 / 10)),
            KeyPair::from(signer),
        ))
        .map_err(|e| {
            println!("Failed to submit step execution tx with error: {:?}", e);
            "FailedToSubmitTransaction"
        })
        .unwrap();
    }
}
