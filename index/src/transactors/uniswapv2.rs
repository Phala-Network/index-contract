use crate::prelude::Error;
use alloc::vec::Vec;
use pink_web3::contract::Options;
use pink_web3::ethabi::Address;
use pink_web3::signing::Key;
use pink_web3::transports::resolve_ready;
use pink_web3::{contract::Contract, keys::pink::KeyPair, transports::PinkHttp};
use primitive_types::{H256, U256};

#[derive(Clone)]
pub struct UniswapV2Client {
    pub contract: Contract<PinkHttp>,
}

impl UniswapV2Client {
    #![allow(clippy::too_many_arguments)]
    pub fn swap(
        &self,
        signer: KeyPair,
        amount_in: U256,
        amount_out: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
        nonce: Option<u64>,
    ) -> core::result::Result<H256, Error> {
        let params = (amount_in, amount_out, path, to, deadline);
        // Estiamte gas before submission
        let gas = resolve_ready(self.contract.estimate_gas(
            "swapExactTokensForTokens",
            params.clone(),
            signer.address(),
            Options::default(),
        ))
        .map_err(|_| Error::FailedToGetGas)?;
        pink_extension::debug!("Estimated swap operation gas cost: {:?}", gas);

        // Actually submit the tx (no guarantee for success)
        let tx_id = resolve_ready(self.contract.signed_call(
            "swapExactTokensForTokens",
            params,
            Options::with(|opt| {
                opt.gas = Some(gas);
                opt.nonce = nonce.map(|nonce| nonce.into());
            }),
            signer,
        ))
        .map_err(|_| Error::FailedToSubmitTransaction)?;

        Ok(tx_id)
    }
}
