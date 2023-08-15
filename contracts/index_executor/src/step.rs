use crate::chain::ChainType;
use crate::utils::ToArray;
use alloc::{borrow::ToOwned, string::String, vec::Vec};
use pink_subrpc::{create_transaction_with_calldata, send_transaction, ExtraParam};

use crate::account::AccountInfo;
use crate::call::{Call, CallParams, SubCall};
use crate::context::Context;
use crate::storage::StorageClient;
use crate::traits::Runner;
use crate::tx;
use pink_web3::{
    api::{Eth, Namespace},
    contract::{Contract, Options},
    keys::pink::KeyPair,
    transports::{resolve_ready, PinkHttp},
    types::U256,
};
use scale::{Decode, Encode};
use serde::Deserialize;

/// The json object that the execution plan consists of
#[derive(Deserialize, Clone)]
pub struct StepJson {
    pub exe_type: String,
    pub exe: String,
    pub source_chain: String,
    pub dest_chain: String,
    pub spend_asset: String,
    pub receive_asset: String,
    pub step_index: u8,
    /// Previous link step's index
    pub previous_step: u8,
    /// Represent the portion of the consumatable amount output of the previous step
    /// value should be in the range of [0, 100]
    pub weight: u8,
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Step {
    pub exe_type: String,
    pub exe: String,
    pub source_chain: String,
    pub dest_chain: String,
    pub spend_asset: Vec<u8>,
    pub receive_asset: Vec<u8>,
    pub sender: Option<Vec<u8>>,
    pub recipient: Option<Vec<u8>>,
    pub spend_amount: Option<u128>,
    // Used to check balance change
    pub origin_balance: Option<u128>,
    pub nonce: Option<u64>,
    pub weight: u8,
    pub calls: Option<Vec<Call>>,
}

impl TryFrom<StepJson> for Step {
    type Error = &'static str;

    fn try_from(json: StepJson) -> Result<Self, Self::Error> {
        Ok(Self {
            exe_type: json.exe_type,
            exe: json.exe,
            source_chain: json.source_chain,
            dest_chain: json.dest_chain,
            spend_asset: Self::decode_address(&json.spend_asset)?,
            receive_asset: Self::decode_address(&json.receive_asset)?,
            sender: None,
            recipient: None,
            spend_amount: None,
            origin_balance: None,
            nonce: None,
            weight: json.weight,
            calls: None,
        })
    }
}

impl Step {
    pub fn derive_calls(&mut self, context: &Context) -> Result<(), &'static str> {
        let action = context
            .get_actions(self.source_chain.clone())
            .ok_or("NoActionFound")?;
        let calls: Vec<Call> = action.build_call(self.clone())?;
        if calls.is_empty() {
            return Err("EmptyCall");
        }
        self.calls = Some(calls);
        Ok(())
    }
}

impl Runner for Step {
    // By checking the nonce of the worker account on the chain source chain we can indicate whether
    // the transaction revalant to the step has been executed.
    fn runnable(
        &self,
        nonce: u64,
        context: &Context,
        _client: Option<&StorageClient>,
    ) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);
        let onchain_nonce = worker_account.get_nonce(self.source_chain.clone(), context)?;
        Ok(onchain_nonce <= nonce)
    }

    fn run(&self, nonce: u64, context: &Context) -> Result<Vec<u8>, &'static str> {
        let signer = context.signer;
        let worker_account = AccountInfo::from(context.signer);
        let chain = context
            .registry
            .get_chain(self.source_chain.clone())
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;

        let calls = self.calls.as_ref().ok_or("MissingCalls")?;

        pink_extension::debug!("Start to execute step with nonce: {}", nonce);
        let tx_id = match chain.chain_type {
            ChainType::Evm => {
                let handler = Contract::from_json(
                    Eth::new(PinkHttp::new(chain.endpoint)),
                    chain.handler_contract.to_array().into(),
                    include_bytes!("./abi/handler.json"),
                )
                .expect("Bad abi data");

                // Estiamte gas before submission
                let gas = resolve_ready(handler.estimate_gas(
                    "batchCall",
                    calls.clone(),
                    worker_account.account20.into(),
                    Options::default(),
                ))
                .map_err(|_| "FailedToEstimateGas")?;
                pink_extension::debug!("Estimated step gas cost: {:?}", gas);

                // Actually submit the tx (no guarantee for success)
                let tx_id = resolve_ready(handler.signed_call(
                    "batchCall",
                    calls.clone(),
                    Options::with(|opt| {
                        opt.gas = Some(gas);
                        opt.nonce = Some(U256::from(nonce));
                    }),
                    KeyPair::from(signer),
                ))
                .map_err(|_| "FailedToSubmitTransaction")?;

                tx_id.as_bytes().to_owned()
            }
            ChainType::Sub => match calls[0].params.clone() {
                CallParams::Sub(SubCall { calldata }) => {
                    let signed_tx = create_transaction_with_calldata(
                        &signer,
                        &chain.name.to_lowercase(),
                        &chain.endpoint,
                        &calldata,
                        ExtraParam {
                            tip: 0,
                            nonce: Some(nonce),
                            era: None,
                        },
                    )
                    .map_err(|_| "InvalidSignature")?;

                    let tx_id = send_transaction(&chain.endpoint, &signed_tx)
                        .map_err(|_| "FailedToSubmitTransaction")?;
                    tx_id
                }
                _ => return Err("UnexpectedCallType"),
            },
        };

        pink_extension::info!(
            "Step execution details: sender,  {:?}, from {:?}, to {:?}, recipient: {:?}, amount: {:?}, tx id: {:?}",
            &hex::encode(&self.sender.clone().ok_or("MissingSender")?),
            &self.source_chain,
            &self.dest_chain,
            &hex::encode(&self.recipient.clone().ok_or("MissingRecipient")?),
            self.spend_amount,
            hex::encode(&tx_id)
        );
        Ok(tx_id)
    }

    // By checking the nonce we can known whether the transaction has been executed or not,
    // and with help of off-chain indexer, we can get the relevant transaction's execution result.
    fn check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);
        let recipient = self.recipient.clone().ok_or("MissingRecipient")?;

        // Query off-chain indexer directly get the execution result
        let chain = &context
            .registry
            .get_chain(self.source_chain.clone())
            .ok_or("MissingChain")?;
        let account = match chain.chain_type {
            ChainType::Evm => worker_account.account20.to_vec(),
            ChainType::Sub => worker_account.account32.to_vec(),
        };
        if tx::check_tx(&chain.tx_indexer, &account, nonce)? {
            // If is a bridge operation, check balance change on dest chain
            if self.exe_type == "bridge" {
                let latest_balance =
                    worker_account.get_balance(self.dest_chain.clone(), recipient, context)?;
                let origin_balance = self.origin_balance.ok_or("MissingOriginReserve")?;

                return Ok(latest_balance > origin_balance);
            }
            return Ok(true);
        }
        Ok(false)
    }
}

impl Step {
    fn u128_from_string(&self, amount: &str) -> Result<u128, &'static str> {
        use fixed::types::U128F0 as Fp;
        let fixed_u128 = Fp::from_str(amount).or(Err("U128ConversionFailed"))?;
        Ok(fixed_u128.to_num())
    }

    fn decode_address(address: &str) -> Result<Vec<u8>, &'static str> {
        if address.len() < 2 && address.len() % 2 != 0 {
            return Err("InvalidAddress");
        }

        hex::decode(&address[2..]).map_err(|_| "DecodeAddressFailed")
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
}
