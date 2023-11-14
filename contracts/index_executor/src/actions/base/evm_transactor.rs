use pink_web3::{
    api::{Eth, Namespace},
    contract::{tokens::Tokenize, Contract},
    ethabi::Address,
    transports::PinkHttp,
    types::U256,
};

use crate::step::Step;
use crate::{
    call::{Call, CallBuilder, CallParams, EvmCall},
    utils::ss58_to_h160,
};

#[derive(Clone)]
pub struct Transactor {
    _eth: Eth<PinkHttp>,
    native_asset: Vec<u8>,
}

impl Transactor {
    pub fn new(rpc: &str, native_asset: Vec<u8>) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        Self {
            _eth: eth,
            native_asset,
        }
    }
}

impl CallBuilder for Transactor {
    fn build_call(&self, step: Step) -> Result<Call, &'static str> {
        let spend_asset = Address::from_slice(&step.spend_asset);
        let spend_amount = U256::from(step.spend_amount.ok_or("MissingSpendAmount")?);
        let recipient = match step.recipient.len() {
            20 => Address::from_slice(&step.recipient),
            32 => Address::from(ss58_to_h160(&step.recipient)),
            _ => return Err("InvalidRecipient"),
        };

        if step.spend_asset == self.native_asset {
            Ok(Call {
                params: CallParams::Evm(EvmCall {
                    target: recipient,
                    calldata: vec![],
                    value: spend_amount,
                    need_settle: true,
                    update_offset: U256::from(0),
                    update_len: U256::from(0),
                    spender: Address::from_slice(&step.sender.ok_or("MissingSender")?),
                    spend_asset,
                    spend_amount,
                    receive_asset: Address::from_slice(&step.receive_asset),
                }),
                input_call: None,
                call_index: None,
            })
        } else {
            let token = Contract::from_json(
                self._eth.clone(),
                Address::from_slice(&step.spend_asset),
                include_bytes!("../../abi/erc20.json"),
            )
            .expect("Bad abi data");
            let transfer_params = (recipient, spend_amount);
            let transfer_func = token
                .abi()
                .function("transfer")
                .map_err(|_| "NoFunctionFound")?;
            let transfer_calldata = transfer_func
                .encode_input(&transfer_params.into_tokens())
                .map_err(|_| "EncodeParamError")?;

            Ok(Call {
                params: CallParams::Evm(EvmCall {
                    target: token.address(),
                    calldata: transfer_calldata,
                    value: U256::from(0),
                    need_settle: true,
                    update_offset: U256::from(20),
                    update_len: U256::from(32),
                    spender: Address::from_slice(&step.sender.ok_or("MissingSender")?),
                    spend_asset,
                    spend_amount,
                    receive_asset: Address::from_slice(&step.receive_asset),
                }),
                input_call: None,
                call_index: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use pink_web3::{contract::Options, transports::resolve_ready};
    use primitive_types::H160;

    use super::*;

    #[test]
    fn test_transfer_dot_on_moonbeam() {
        pink_extension_runtime::mock_ext::mock_all_ext();
        let rpc = "https://moonbeam.api.onfinality.io/public";
        let handler_address: H160 =
            H160::from_slice(&hex::decode("B8D20dfb8c3006AA17579887ABF719DA8bDf005B").unwrap());
        let transport = Eth::new(PinkHttp::new(rpc.clone()));
        let handler = Contract::from_json(
            transport,
            handler_address,
            include_bytes!("../../abi/handler.json"),
        )
        .unwrap();
        let transactor = Transactor::new(rpc, vec![0u8; 20]);
        let mut call = transactor
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Moonbeam"),
                dest_chain: String::from("Moonbeam"),
                spend_asset: hex::decode("FfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080").unwrap(),
                receive_asset: hex::decode("FfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080").unwrap(),
                sender: Some(hex::decode("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").unwrap()),
                recipient: hex::decode("7e61a916C841a4ebf2F3AA1298C0Ad2e503816e3").unwrap(),
                // 1 DOT
                spend_amount: Some(1_000_000_000_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        call.input_call = Some(0);
        call.call_index = Some(0);

        match &call.params {
            CallParams::Evm(evm_call) => {
                println!("calldata: {:?}", hex::encode(&evm_call.calldata))
            }
            _ => assert!(false),
        }

        let gas = resolve_ready(handler.estimate_gas(
            "batchCall",
            vec![call.clone()],
            // Worker address
            Address::from_slice(&hex::decode("5cddb3ad187065e0122f3f46d13ad6ca486e4644").unwrap()),
            Options::default(),
        ))
        .map_err(|e| {
            println!("Failed to estimated step gas cost with error: {:?}", e);
            "FailedToEstimateGas"
        })
        .unwrap();

        println!("gas: {:?}", gas);
    }

    #[test]
    fn test_transfer_glmr_on_moonbeam() {
        pink_extension_runtime::mock_ext::mock_all_ext();
        let rpc = "https://moonbeam.api.onfinality.io/public";
        let handler_address: H160 =
            H160::from_slice(&hex::decode("B8D20dfb8c3006AA17579887ABF719DA8bDf005B").unwrap());
        let transport = Eth::new(PinkHttp::new(rpc.clone()));
        let handler = Contract::from_json(
            transport,
            handler_address,
            include_bytes!("../../abi/handler.json"),
        )
        .unwrap();
        let transactor = Transactor::new(rpc, vec![0u8; 20]);
        let mut call = transactor
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Moonbeam"),
                dest_chain: String::from("Moonbeam"),
                spend_asset: hex::decode("0000000000000000000000000000000000000000").unwrap(),
                receive_asset: hex::decode("0000000000000000000000000000000000000000").unwrap(),
                sender: Some(hex::decode("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").unwrap()),
                recipient: hex::decode("7e61a916C841a4ebf2F3AA1298C0Ad2e503816e3").unwrap(),
                // 1 GLMR
                spend_amount: Some(1_000_000_000_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        call.input_call = Some(0);
        call.call_index = Some(0);

        match &call.params {
            CallParams::Evm(evm_call) => {
                println!("calldata: {:?}", evm_call.value)
            }
            _ => assert!(false),
        }

        let gas = resolve_ready(handler.estimate_gas(
            "batchCall",
            vec![call.clone()],
            // Worker address
            Address::from_slice(&hex::decode("5cddb3ad187065e0122f3f46d13ad6ca486e4644").unwrap()),
            Options::default(),
        ))
        .map_err(|e| {
            println!("Failed to estimated step gas cost with error: {:?}", e);
            "FailedToEstimateGas"
        })
        .unwrap();

        println!("gas: {:?}", gas);
    }
}
