use alloc::vec;
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
pub struct UniswapV2 {
    pub eth: Eth<PinkHttp>,
    pub router: Contract<PinkHttp>,
}

impl UniswapV2 {
    pub fn new(rpc: &str, router: Address) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let router = Contract::from_json(
            eth.clone(),
            router,
            include_bytes!("../../abi/UniswapV2Router02.json"),
        )
        .expect("Bad abi data");

        Self { eth, router }
    }
}

impl CallBuilder for UniswapV2 {
    fn build_call(&self, step: Step) -> Result<Call, &'static str> {
        let asset0 = Address::from_slice(&step.spend_asset);
        let asset1 = Address::from_slice(&step.receive_asset);
        let to = Address::from_slice(&step.recipient.ok_or("MissingRecipient")?);
        let path = vec![asset0, asset1];
        let amount_out = U256::from(1);
        let amount_in = U256::from(step.spend_amount.ok_or("MissingSpendAmount")?);
        let time = pink_extension::ext().untrusted_millis_since_unix_epoch() / 1000;
        // 1 month
        let deadline = U256::from(time + 60 * 60 * 24 * 30);
        let swap_params = (amount_in, amount_out, path, to, deadline);
        let swap_func = self
            .router
            .abi()
            .function("swapExactTokensForTokens")
            .map_err(|_| "NoFunctionFound")?;
        let swap_calldata = swap_func
            .encode_input(&swap_params.into_tokens())
            .map_err(|_| "EncodeParamError")?;

        Ok(Call {
            params: CallParams::Evm(EvmCall {
                target: self.router.address(),
                calldata: swap_calldata,
                value: U256::from(0),

                need_settle: true,
                update_offset: U256::from(4),
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
