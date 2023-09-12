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
pub struct UniswapV3 {
    pub eth: Eth<PinkHttp>,
    pub router: Contract<PinkHttp>,
}

impl UniswapV3 {
    pub fn new(rpc: &str, router: Address) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let router = Contract::from_json(
            eth.clone(),
            router,
            include_bytes!("../../abi/uniswapV3Router.json"),
        )
        .expect("Bad abi data");

        Self { eth, router }
    }
}

impl CallBuilder for UniswapV3 {
    fn build_call(&self, step: Step) -> Result<Call, &'static str> {
        let asset0 = Address::from_slice(&step.spend_asset);
        let asset1 = Address::from_slice(&step.receive_asset);
        let to = Address::from_slice(&step.recipient.ok_or("MissingRecipient")?);
        let amount_out = U256::from(1);
        let amount_in = U256::from(step.spend_amount.ok_or("MissingSpendAmount")?);
        let time = pink_extension::ext().untrusted_millis_since_unix_epoch() / 1000;
        // 1 month
        let deadline = U256::from(time + 60 * 60 * 24 * 30);
        let swap_params = (asset0, asset1, to, deadline, amount_in, amount_out, 0_u128);
        // https://github.com/Uniswap/v3-periphery/blob/6cce88e63e176af1ddb6cc56e029110289622317/contracts/SwapRouter.sol#L115
        let swap_func = self
            .router
            .abi()
            .function("exactInputSingle")
            .map_err(|_| "NoFunctionFound")?;
        let swap_calldata = swap_func
            .encode_input(&[Token::Tuple(swap_params.into_tokens())])
            .map_err(|_| "EncodeParamError")?;

        Ok(Call {
            params: CallParams::Evm(EvmCall {
                target: self.router.address(),
                calldata: swap_calldata,
                value: U256::from(0),

                need_settle: true,
                update_offset: U256::from(132),
                update_len: U256::from(32),
                spender: self.router.address(),
                spend_asset: asset0,
                spend_amount: amount_in,
                receive_asset: asset1,
            }),
            input_call: None,
            call_index: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            H160::from_slice(&hex::decode("635eA86804200F80C16ea8EdDc3c749a54a9C37D").unwrap());
        let transport = Eth::new(PinkHttp::new("https://rpc.api.moonbeam.network"));
        let handler = Contract::from_json(
            transport,
            handler_address,
            include_bytes!("../../abi/handler.json"),
        )
        .unwrap();
        let stellaswap_routerv3: [u8; 20] = hex::decode("e6d0ED3759709b743707DcfeCAe39BC180C981fe")
            .unwrap()
            .to_array();
        let stellaswap_v3 = UniswapV3::new(
            "https://rpc.api.moonbeam.network",
            stellaswap_routerv3.into(),
        );
        let mut call = stellaswap_v3
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Moonbeam"),
                dest_chain: String::from("Moonbeam"),
                // xcDOT
                spend_asset: hex::decode("FfFFfFff1FcaCBd218EDc0EbA20Fc2308C778080").unwrap(),
                // xcPHA
                receive_asset: hex::decode("FFFfFfFf63d24eCc8eB8a7b5D0803e900F7b6cED").unwrap(),
                sender: None,
                recipient: Some(hex::decode("A29D4E0F035cb50C0d78c8CeBb56Ca292616Ab20").unwrap()),
                // 0.002 xcDOT
                spend_amount: Some(2_0_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        // Apply index mannually
        call.input_call = Some(0);
        call.call_index = Some(0);

        // Estiamte gas before submission
        let gas = resolve_ready(handler.estimate_gas(
            "batchCall",
            call.clone(),
            Address::from_slice(&hex::decode("bf526928373748b00763875448ee905367d97f96").unwrap()),
            Options::default(),
        ))
        .map_err(|e| {
            println!("Failed to estimated step gas cost with error: {:?}", e);
            "FailedToEstimateGas"
        })
        .unwrap();

        // Uncomment if wanna send it to blockchain
        let _tx_id = resolve_ready(handler.signed_call(
            "batchCall",
            call,
            Options::with(|opt| {
                opt.gas = Some(gas);
            }),
            KeyPair::from(signer),
        ))
        .map_err(|e| {
            println!("Failed to submit step execution tx with error: {:?}", e);
            "FailedToSubmitTransaction"
        })
        .unwrap();
    }
}
