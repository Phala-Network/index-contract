use crate::chain::{BalanceFetcher, Chain, ChainType};
use crate::utils::ToArray;
use alloc::vec;
use alloc::{borrow::ToOwned, boxed::Box, string::String, vec::Vec};
use pink_subrpc::{create_transaction_with_calldata, send_transaction, ExtraParam};

use crate::account::AccountInfo;
use crate::call::{Call, CallBuilder, CallParams, SubCall};
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
            spend_amount: Some(0),
            origin_balance: None,
            nonce: None,
        })
    }
}

impl Step {
    pub fn get_action(&mut self, context: &Context) -> Result<Box<dyn CallBuilder>, &'static str> {
        let action = context
            .get_actions(self.source_chain.clone())
            .ok_or("NoActionFound")?;
        Ok(action)
    }

    pub fn derive_calls(&self, context: &Context) -> Result<Vec<Call>, &'static str> {
        let action = context
            .get_actions(self.source_chain.clone())
            .ok_or("NoActionFound")?;

        action.build_call(self.clone())
    }

    pub fn is_bridge_step(&self) -> bool {
        self.source_chain.to_lowercase() != self.dest_chain.to_lowercase()
    }

    pub fn source_chain(&self, context: &Context) -> Option<Chain> {
        context.registry.get_chain(self.source_chain.clone())
    }

    pub fn dest_chain(&self, context: &Context) -> Option<Chain> {
        context.registry.get_chain(self.dest_chain.clone())
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

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum MultiStep {
    Single(Step),
    Batch(Vec<Step>),
}

impl MultiStep {
    pub fn derive_calls(&mut self, context: &Context) -> Result<Vec<Call>, &'static str> {
        if self.as_single_step().spend_amount.is_none() {
            return Err("MissingSpendAmount");
        }
        let calls = match self {
            MultiStep::Single(step) => step.derive_calls(context)?,
            MultiStep::Batch(batch_steps) => {
                if batch_steps.is_empty() {
                    return Err("BatchStepEmpty");
                }

                let mut calls: Vec<Call> = vec![];
                for step in batch_steps.iter() {
                    let origin_call_count = calls.len();
                    let mut next_call_index = origin_call_count.try_into().expect("Too many calls");
                    let mut new_calls = step.derive_calls(context)?;
                    for call in new_calls.iter_mut() {
                        call.call_index = Some(next_call_index);
                        call.input_call = calls[origin_call_count - 1].call_index;
                        next_call_index += 1;
                    }
                    calls.append(&mut new_calls);
                }
                calls
            }
        };
        if calls.is_empty() {
            return Err("EmptyCall");
        }

        Ok(calls)
    }

    pub fn as_single_step(&self) -> Step {
        match self {
            MultiStep::Single(step) => step.clone(),
            MultiStep::Batch(batch_steps) => {
                let mut first_step = batch_steps[0].clone();
                let last_step = batch_steps[batch_steps.len() - 1].clone();
                first_step.receive_asset = last_step.receive_asset;
                first_step.recipient = last_step.recipient;
                first_step.clone()
            }
        }
    }

    pub fn set_spend(&mut self, amount: u128) {
        match self {
            MultiStep::Single(step) => {
                step.spend_amount = Some(amount);
            }
            MultiStep::Batch(batch_steps) => {
                let first_step = &mut batch_steps[0];
                first_step.spend_amount = Some(amount)
            }
        }
    }

    pub fn settle(&self, context: &Context) -> Result<u128, &'static str> {
        let step = self.as_single_step();
        let dest_chain = &context
            .registry
            .get_chain(step.dest_chain.clone())
            .ok_or("MissingChain")?;
        let origin_balance = step.origin_balance.ok_or("MissingBalance")?;
        let recipient = step.recipient.clone().ok_or("MissingRecipient")?;
        let latest_balance = dest_chain.get_balance(step.receive_asset.clone(), recipient)?;
        Ok(latest_balance.saturating_sub(origin_balance))
    }

    pub fn sync_origin_balance(&mut self, context: &Context) -> Result<(), &'static str> {
        let (recipient, receive_asset, dest_chain) = match self {
            MultiStep::Single(step) => (
                step.recipient.clone(),
                step.receive_asset.clone(),
                step.dest_chain.clone(),
            ),
            MultiStep::Batch(batch_steps) => {
                let last_step = batch_steps[batch_steps.len() - 1].clone();
                (
                    last_step.recipient,
                    last_step.receive_asset,
                    last_step.dest_chain,
                )
            }
        };

        let chain = &context
            .registry
            .get_chain(dest_chain)
            .ok_or("MissingChain")?;
        let origin_balance =
            chain.get_balance(receive_asset, recipient.ok_or("MissingRecipient")?)?;

        match self {
            MultiStep::Single(step) => {
                step.origin_balance = Some(origin_balance);
            }
            MultiStep::Batch(batch_steps) => {
                let first_step = &mut batch_steps[0];
                first_step.origin_balance = Some(origin_balance)
            }
        }

        Ok(())
    }

    pub fn set_nonce(&mut self, nonce: u64) {
        match self {
            MultiStep::Single(step) => {
                step.nonce = Some(nonce);
            }
            MultiStep::Batch(batch_steps) => {
                let first_step = &mut batch_steps[0];
                first_step.nonce = Some(nonce)
            }
        }
    }

    pub fn get_nonce(&self) -> Option<u64> {
        match self {
            MultiStep::Single(step) => step.nonce,
            MultiStep::Batch(batch_steps) => {
                let first_step = &batch_steps[0];
                first_step.nonce
            }
        }
    }
}

impl Runner for MultiStep {
    // By checking the nonce of the worker account on the chain source chain we can indicate whether
    // the transaction revalant to the step has been executed.
    fn runnable(
        &self,
        nonce: u64,
        context: &Context,
        _client: Option<&StorageClient>,
    ) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);
        let onchain_nonce =
            worker_account.get_nonce(self.as_single_step().source_chain.clone(), context)?;
        Ok(onchain_nonce <= nonce)
    }

    fn run(&mut self, nonce: u64, context: &Context) -> Result<Vec<u8>, &'static str> {
        let as_single_step = self.as_single_step();
        let signer = context.signer;
        let worker_account = AccountInfo::from(context.signer);
        let chain = context
            .registry
            .get_chain(as_single_step.source_chain.clone())
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;
        let calls = self.derive_calls(context)?;

        self.sync_origin_balance(context)?;

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
            &hex::encode(&as_single_step.sender.clone().ok_or("MissingSender")?),
            &as_single_step.source_chain,
            &as_single_step.dest_chain,
            &hex::encode(&as_single_step.recipient.clone().ok_or("MissingRecipient")?),
            as_single_step.spend_amount,
            hex::encode(&tx_id)
        );
        Ok(tx_id)
    }

    // By checking the nonce we can known whether the transaction has been executed or not,
    // and with help of off-chain indexer, we can get the relevant transaction's execution result.
    fn check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        let as_single_step = self.as_single_step();
        let worker_account = AccountInfo::from(context.signer);
        let recipient = as_single_step.recipient.clone().ok_or("MissingRecipient")?;

        // Query off-chain indexer directly get the execution result
        let chain = &context
            .registry
            .get_chain(as_single_step.source_chain.clone())
            .ok_or("MissingChain")?;
        let account = match chain.chain_type {
            ChainType::Evm => worker_account.account20.to_vec(),
            ChainType::Sub => worker_account.account32.to_vec(),
        };
        if tx::check_tx(&chain.tx_indexer, &account, nonce)? {
            // If is a bridge operation, check balance change on dest chain
            if as_single_step.exe_type == "bridge" {
                let latest_balance =
                    chain.get_balance(as_single_step.receive_asset.clone(), recipient)?;
                let origin_balance = as_single_step
                    .origin_balance
                    .ok_or("MissingOriginReserve")?;

                return Ok(latest_balance > origin_balance);
            }
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
}
