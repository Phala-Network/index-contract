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
pub struct XTokenBridge {
    eth: Eth<PinkHttp>,
    xtoken: Contract<PinkHttp>,
    dest_chain_id: u32,
}

impl XTokenBridge {
    pub fn new(rpc: &str, xtoken_address: Address, dest_chain_id: u32) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let xtoken = Contract::from_json(
            eth.clone(),
            xtoken_address,
            include_bytes!("../../abi/xtokens-abi.json"),
        )
        .expect("Bad abi data");

        Self {
            eth,
            xtoken,
            dest_chain_id,
        }
    }
}

impl CallBuilder for XTokenBridge {
    fn build_call(&self, step: Step) -> Result<Vec<Call>, &'static str> {
        let spend_asset = Address::from_slice(&step.spend_asset);
        // We don't use it
        let receive_asset = Address::from_slice(&[0; 20]);
        let mut recipient = step.recipient.ok_or("MissingRecipient")?;
        let spend_amount = U256::from(step.spend_amount.ok_or("MissingSpendAmount")?);

        let weight: u64 = 6000000000;
        let location = Token::Tuple(vec![
            Token::Uint(1_u8.into()),
            Token::Array(vec![
                Token::Bytes(
                    // Parachain(#[codec(compact)] u32),
                    {
                        let mut bytes: Vec<u8> = vec![];
                        let mut enum_id = 0_u8.to_be_bytes().to_vec();
                        let mut chain_id = self.dest_chain_id.to_be_bytes().to_vec();
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
        ]);
        let bridge_params = (spend_asset, spend_amount, location, weight);

        let bridge_func = self
            .xtoken
            .abi()
            .function("transfer")
            .map_err(|_| "NoFunctionFound")?;
        let bridge_calldata = bridge_func
            .encode_input(&bridge_params.into_tokens())
            .map_err(|_| "EncodeParamError")?;

        let token = Contract::from_json(
            self.eth.clone(),
            spend_asset,
            include_bytes!("../../abi/erc20.json"),
        )
        .expect("Bad abi data");
        let approve_params = (self.xtoken.address(), spend_amount);
        let approve_func = token
            .abi()
            .function("approve")
            .map_err(|_| "NoFunctionFound")?;
        let approve_calldata = approve_func
            .encode_input(&approve_params.into_tokens())
            .map_err(|_| "EncodeParamError")?;

        Ok(vec![
            Call {
                params: CallParams::Evm(EvmCall {
                    target: spend_asset,
                    calldata: approve_calldata,
                    value: U256::from(0),

                    need_settle: false,
                    update_offset: U256::from(36),
                    update_len: U256::from(32),
                    spend_asset,
                    spend_amount,
                    receive_asset,
                }),
                input_call: None,
                call_index: None,
            },
            Call {
                params: CallParams::Evm(EvmCall {
                    target: self.xtoken.address(),
                    calldata: bridge_calldata,
                    value: U256::from(0),

                    // Bridge operation do not need do settlement on source chain, because it must be the
                    // last step on source chain
                    need_settle: false,
                    update_offset: U256::from(36),
                    update_len: U256::from(32),
                    spend_asset,
                    spend_amount,
                    receive_asset,
                }),
                input_call: None,
                call_index: None,
            },
        ])
    }
}
