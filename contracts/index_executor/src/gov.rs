use alloc::string::String;
use alloc::vec::Vec;
use pink_web3::{
    api::{Eth, Namespace},
    contract::{Contract, Options},
    keys::pink::KeyPair,
    signing::Key,
    transports::{resolve_ready, PinkHttp},
    types::{Address, U256},
};

use crate::task::TaskId;

pub struct WorkerGov;

impl WorkerGov {
    pub fn drop_task(
        worker_key: [u8; 32],
        endpoint: String,
        handler: Address,
        id: TaskId,
    ) -> Result<Vec<u8>, &'static str> {
        let transport = Eth::new(PinkHttp::new(endpoint));
        let handler_contract =
            Contract::from_json(transport, handler, include_bytes!("./abi/handler.json"))
                .or(Err("ConstructContractFailed"))?;
        let worker = KeyPair::from(worker_key);

        let gas = resolve_ready(handler_contract.estimate_gas(
            "drop",
            id,
            worker.address(),
            Options::default(),
        ))
        .or(Err("GasEstimateFailed"))?;

        // Submit the `approve` transaction
        let tx_id = resolve_ready(handler_contract.signed_call(
            "drop",
            id,
            Options::with(|opt| {
                opt.gas = Some(gas);
            }),
            worker,
        ))
        .or(Err("ERC20ApproveSubmitFailed"))?;
        pink_extension::info!(
            "Submit transaction to do task drop, task {:?} , tx id: {:?}",
            hex::encode(id),
            hex::encode(tx_id.clone().as_bytes())
        );

        Ok(tx_id.as_bytes().to_vec())
    }

    pub fn erc20_approve(
        worker_key: [u8; 32],
        endpoint: String,
        token: Address,
        spender: Address,
        amount: u128,
    ) -> Result<Vec<u8>, &'static str> {
        let transport = Eth::new(PinkHttp::new(endpoint));
        let erc20_token = Contract::from_json(transport, token, include_bytes!("./abi/erc20.json"))
            .or(Err("ConstructContractFailed"))?;
        let worker = KeyPair::from(worker_key);

        // Estiamte gas before submission
        let gas = resolve_ready(erc20_token.estimate_gas(
            "approve",
            (spender, U256::from(amount)),
            worker.address(),
            Options::default(),
        ))
        .or(Err("GasEstimateFailed"))?;

        // Submit the `approve` transaction
        let tx_id = resolve_ready(erc20_token.signed_call(
            "approve",
            (spender, U256::from(amount)),
            Options::with(|opt| {
                opt.gas = Some(gas);
            }),
            worker,
        ))
        .or(Err("ERC20ApproveSubmitFailed"))?;
        pink_extension::info!(
            "Submit transaction to do ERC20 approve, token {:?} , spender {:?}, amount: {:?}, tx id: {:?}",
            hex::encode(token),
            hex::encode(spender),
            amount,
            hex::encode(tx_id.clone().as_bytes())
        );

        Ok(tx_id.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use dotenv::dotenv;
    use hex_literal::hex;
    use index::utils::ToArray;
    use pink_web3::types::H160;

    #[test]
    #[ignore]
    fn test_worker_approve() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();
        let pha_on_goerli: H160 = hex!("b376b0ee6d8202721838e76376e81eec0e2fe864").into();
        let spender: H160 = hex!("ffffffffffffffffffffffffffffffffffffffff").into();

        // Issue command: SECRET_KEY=<private key> cargo test --package index_executor --lib -- gov::tests::test_worker_approve --exact --nocapture
        // Send approve transaction
        // https://goerli.etherscan.io/tx/0x6d711af99d4836c8febe3d27e14bc0ad9b8353d89ebcae3f465a6cc70519e35c
        let tx_id = WorkerGov::erc20_approve(
            signer,
            String::from("https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2"),
            pha_on_goerli,
            spender,
            // 100 PHA
            100_000_000_000_000_000_000_u128,
        )
        .unwrap();
        dbg!("Approve transaction: {:?}", hex::encode(tx_id));
    }
}
