use pink_web3::{
    api::{Eth, Namespace},
    contract::{tokens::Tokenize, Contract},
    ethabi::Address,
    transports::PinkHttp,
    types::U256,
};

use crate::call::{Call, CallBuilder, CallParams, EvmCall};
use crate::step::Step;

#[derive(Clone)]
pub struct NativeWrapper {
    pub eth: Eth<PinkHttp>,
    pub weth9: Contract<PinkHttp>,
    pub native: Address,
}

impl NativeWrapper {
    pub fn new(rpc: &str, weth9: Address, native: Address) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let weth9 = Contract::from_json(eth.clone(), weth9, include_bytes!("../../abi/WETH9.json"))
            .expect("Bad abi data");

        Self { eth, weth9, native }
    }
}

impl CallBuilder for NativeWrapper {
    fn build_call(&self, step: Step) -> Result<Call, &'static str> {
        let spend_asset = Address::from_slice(&step.spend_asset);
        let receive_asset = Address::from_slice(&step.receive_asset);
        let spend_amount = U256::from(step.spend_amount.ok_or("MissingSpendAmount")?);

        if spend_asset == self.native && receive_asset == self.weth9.address() {
            // Deposit
            let deposit_func = self
                .weth9
                .abi()
                .function("deposit")
                .map_err(|_| "NoFunctionFound")?;
            let deposit_calldata = deposit_func
                .encode_input(&[])
                .map_err(|_| "EncodeParamError")?;
            Ok(Call {
                params: CallParams::Evm(EvmCall {
                    target: self.weth9.address(),
                    calldata: deposit_calldata,
                    value: spend_amount,

                    need_settle: true,
                    update_offset: 0,
                    update_len: 0,
                    // No spender
                    spender: Address::from(&[0; 20]),
                    spend_asset,
                    spend_amount,
                    receive_asset,
                }),
                input_call: None,
                call_index: None,
            })
        } else if spend_asset == self.weth9.address() && receive_asset == self.native {
            // Withdraw
            let withdraw_func = self
                .weth9
                .abi()
                .function("withdraw")
                .map_err(|_| "NoFunctionFound")?;
            let withdraw_calldata = withdraw_func
                .encode_input(&spend_amount.into_tokens())
                .map_err(|_| "EncodeParamError")?;
            Ok(Call {
                params: CallParams::Evm(EvmCall {
                    target: self.weth9.address(),
                    calldata: withdraw_calldata,
                    value: U256::from(0),

                    need_settle: true,
                    update_offset: 4,
                    update_len: 32,
                    // No spender
                    spender: Address::from(&[0; 20]),
                    spend_asset,
                    spend_amount,
                    receive_asset,
                }),
                input_call: None,
                call_index: None,
            })
        } else {
            Err("UnrecognizedArguments")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::PackCall;
    use crate::utils::ToArray;
    use dotenv::dotenv;
    use primitive_types::H160;

    use pink_web3::{
        api::{Eth, Namespace},
        contract::{Contract, Options},
        keys::pink::KeyPair,
        transports::{resolve_ready, PinkHttp},
    };

    #[test]
    #[ignore]
    fn should_work() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();
        use pink_web3::types::Address;

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        // Handler on Moonbeam
        let handler_address: H160 =
            H160::from_slice(&hex::decode("50a0D445E2Df1255e01B27F932D90421305a8eCA").unwrap());
        let transport = Eth::new(PinkHttp::new("https://rpc.api.moonbeam.network"));
        let handler = Contract::from_json(
            transport,
            handler_address,
            include_bytes!("../../abi/handler.json"),
        )
        .unwrap();

        let wglmr: [u8; 20] = hex::decode("Acc15dC74880C9944775448304B263D191c6077F")
            .unwrap()
            .to_array();
        let glmr: [u8; 20] = hex::decode("0000000000000000000000000000000000000802")
            .unwrap()
            .to_array();
        let native_wrapper: NativeWrapper = NativeWrapper::new(
            "https://rpc.api.moonbeam.network",
            wglmr.into(),
            glmr.into(),
        );
        let mut deposit_call = native_wrapper
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Moonbeam"),
                dest_chain: String::from("Moonbeam"),
                // glmr
                spend_asset: glmr.into(),
                // wglmr
                receive_asset: wglmr.into(),
                sender: None,
                recipient: Some(hex::decode("bf526928373748b00763875448ee905367d97f96").unwrap()),
                // 0.1 glmr
                spend_amount: Some(100_000_000_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        let mut withdraw_call = native_wrapper
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Moonbeam"),
                dest_chain: String::from("Moonbeam"),
                // wglmr
                spend_asset: wglmr.into(),
                // glmr
                receive_asset: glmr.into(),
                sender: None,
                recipient: Some(hex::decode("bf526928373748b00763875448ee905367d97f96").unwrap()),
                // 0.05 wglmr, will be updated to 0.1 wglmr bc we set deposit call as input call where we got 0.1 wglmr
                spend_amount: Some(50_000_000_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        // Apply index mannually
        deposit_call.input_call = Some(0);
        deposit_call.call_index = Some(0);
        withdraw_call.input_call = Some(0);
        withdraw_call.call_index = Some(1);

        let calls = [deposit_call, withdraw_call].to_vec();

        // Estiamte gas before submission
        let gas = resolve_ready(handler.estimate_gas(
            "batchCall",
            calls.clone().pack(),
            Address::from_slice(&hex::decode("bf526928373748b00763875448ee905367d97f96").unwrap()),
            Options::with(|opt| {
                // 0.1 GLMR
                opt.value = Some(U256::from(100_000_000_000_000_000_u128));
            }),
        ))
        .map_err(|e| {
            println!("Failed to estimated step gas cost with error: {:?}", e);
            "FailedToEstimateGas"
        })
        .unwrap();

        // test tx: https://moonscan.io/tx/0x5d10ef2d1232d8047a9f8ec3fba4048f4b262079ea8de761fb7d3f0a966a0e7e
        // Uncomment if wanna send it to blockchain
        let tx_id = resolve_ready(handler.signed_call(
            "batchCall",
            calls.pack(),
            Options::with(|opt| {
                opt.gas = Some(gas);
                // 0.1 GLMR
                opt.value = Some(U256::from(100_000_000_000_000_000_u128));
            }),
            KeyPair::from(signer),
        ))
        .map_err(|e| {
            println!("Failed to submit step execution tx with error: {:?}", e);
            "FailedToSubmitTransaction"
        })
        .unwrap();
        println!("native warpper test tx: {:?}", tx_id);
    }
}
