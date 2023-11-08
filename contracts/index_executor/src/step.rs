use crate::actions::ActionExtraInfo;
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

#[derive(Clone, Debug, Decode, Encode, PartialEq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct StepInput {
    pub exe: String,
    pub source_chain: String,
    pub dest_chain: String,
    pub spend_asset: String,
    pub receive_asset: String,
    pub recipient: String,
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Step {
    pub exe: String,
    pub source_chain: String,
    pub dest_chain: String,
    pub spend_asset: Vec<u8>,
    pub receive_asset: Vec<u8>,
    pub sender: Option<Vec<u8>>,
    pub recipient: Vec<u8>,
    pub spend_amount: Option<u128>,
    // Used to check balance change
    pub origin_balance: Option<u128>,
    pub nonce: Option<u64>,
}

impl sp_std::fmt::Debug for Step {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
        f.debug_struct("Step")
            .field("exe", &self.exe)
            .field("source_chain", &self.source_chain)
            .field("dest_chain", &self.dest_chain)
            .field("spend_asset", &hex::encode(&self.spend_asset))
            .field("receive_asset", &hex::encode(&self.receive_asset))
            .field("sender", &self.sender.as_ref().map(hex::encode))
            .field("recipient", &hex::encode(&self.recipient))
            .field("spend_amount", &self.spend_amount)
            .field("origin_balance", &self.origin_balance)
            .field("nonce", &self.nonce)
            .finish()
    }
}

impl TryFrom<StepInput> for Step {
    type Error = &'static str;

    fn try_from(input: StepInput) -> Result<Self, Self::Error> {
        Ok(Self {
            exe: input.exe,
            source_chain: input.source_chain,
            dest_chain: input.dest_chain,
            spend_asset: Self::decode_address(&input.spend_asset)?,
            receive_asset: Self::decode_address(&input.receive_asset)?,
            sender: None,
            recipient: Self::decode_address(&input.recipient)?,
            spend_amount: Some(0),
            origin_balance: None,
            nonce: None,
        })
    }
}

impl Step {
    pub fn get_action(&self, context: &Context) -> Result<Box<dyn CallBuilder>, &'static str> {
        let action = context
            .get_actions(&self.source_chain, &self.exe)
            .ok_or("NoActionFound")?;
        pink_extension::debug!("Found action: {:?} on {:?}", &self.exe, &self.source_chain,);
        Ok(action)
    }

    pub fn derive_calls(&self, context: &Context) -> Result<Vec<Call>, &'static str> {
        let action = self.get_action(context)?;
        pink_extension::debug!(
            "Trying to build calldata for according to step data: {:?}",
            self,
        );
        let source_chain = self.source_chain(context).ok_or("MissingSourceChain")?;
        let worker_account = AccountInfo::from(context.signer);
        let sender = match source_chain.chain_type {
            ChainType::Evm => worker_account.account20.to_vec(),
            ChainType::Sub => worker_account.account32.to_vec(),
        };

        let mut step = self.clone();
        step.sender = Some(sender);
        let call = action.build_call(step)?;
        Ok(vec![call])
    }

    pub fn is_bridge_step(&self) -> bool {
        self.source_chain.to_lowercase() != self.dest_chain.to_lowercase()
    }

    pub fn source_chain(&self, context: &Context) -> Option<Chain> {
        context.registry.get_chain(&self.source_chain)
    }

    pub fn dest_chain(&self, context: &Context) -> Option<Chain> {
        context.registry.get_chain(&self.dest_chain)
    }
}

impl Step {
    fn decode_address(address: &str) -> Result<Vec<u8>, &'static str> {
        if address.len() < 2 && address.len() % 2 != 0 {
            return Err("InvalidAddressInStep");
        }

        hex::decode(&address[2..]).map_err(|_| "DecodeAddressFailed")
    }
}

#[derive(Clone, Debug, Decode, Encode, PartialEq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[allow(clippy::large_enum_variant)]
pub enum MultiStepInput {
    Single(StepInput),
    Batch(Vec<StepInput>),
}

impl TryFrom<MultiStepInput> for MultiStep {
    type Error = &'static str;

    fn try_from(input: MultiStepInput) -> Result<Self, Self::Error> {
        match input {
            MultiStepInput::Single(step_input) => {
                Ok(MultiStep::Single(Step::try_from(step_input)?))
            }
            MultiStepInput::Batch(vec_step_input) => {
                let mut vec_step = Vec::new();
                for step_input in vec_step_input {
                    vec_step.push(Step::try_from(step_input)?);
                }
                Ok(MultiStep::Batch(vec_step))
            }
        }
    }
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
// TODO: consider box Step
#[allow(clippy::large_enum_variant)]
pub enum MultiStep {
    Single(Step),
    Batch(Vec<Step>),
}

impl MultiStep {
    pub fn derive_calls(&self, context: &Context) -> Result<Vec<Call>, &'static str> {
        if self.as_single_step().spend_amount.is_none() {
            return Err("MissingSpendAmount");
        }
        let calls = match self {
            MultiStep::Single(step) => {
                let mut calls = step.derive_calls(context)?;
                assert!(calls.len() == 1);
                calls[0].call_index = Some(0);
                calls[0].input_call = Some(0);
                calls
            }
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
                        call.input_call = if origin_call_count > 0 {
                            calls[origin_call_count - 1].call_index
                        } else {
                            Some(0)
                        };
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
                first_step.dest_chain = last_step.dest_chain;
                first_step.receive_asset = last_step.receive_asset;
                first_step.recipient = last_step.recipient;
                first_step
            }
        }
    }

    pub fn is_single_step(&self) -> bool {
        matches!(self, MultiStep::Single(_))
    }

    pub fn is_batch_step(&self) -> bool {
        matches!(self, MultiStep::Batch(_))
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
        let dest_chain = step.dest_chain(context).ok_or("MissingDestChain")?;
        let origin_balance = step.origin_balance.ok_or("MissingBalance")?;
        let recipient = step.recipient.clone();
        let latest_balance = dest_chain.get_balance(step.receive_asset, recipient.clone())?;
        pink_extension::debug!(
            "Settle info of account {:?}: origin_balance is {:?}, latest_balance is {:?}",
            &hex::encode(&recipient),
            origin_balance,
            latest_balance
        );
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
            .get_chain(&dest_chain)
            .ok_or("MissingDestChain")?;
        let origin_balance = chain.get_balance(receive_asset, recipient)?;

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
            worker_account.get_nonce(&self.as_single_step().source_chain, context)?;
        Ok(onchain_nonce <= nonce)
    }

    fn run(&mut self, nonce: u64, context: &Context) -> Result<Vec<u8>, &'static str> {
        let as_single_step = self.as_single_step();
        let chain = as_single_step
            .source_chain(context)
            .ok_or("MissingSourceChain")?;
        let signer = context.signer;
        let worker_account = AccountInfo::from(context.signer);
        let calls = self.derive_calls(context)?;
        pink_extension::debug!("Derived calls to be sumitted: {:?}", &calls);

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
                .map_err(|e| {
                    pink_extension::error!("Failed to estimated step gas cost with error: {:?}", e);
                    "FailedToEstimateGas"
                })?;
                pink_extension::debug!("Estimated step gas: {:?}", gas);

                // Actually submit the tx (no guarantee for success)
                let tx_id = resolve_ready(handler.signed_call(
                    "batchCall",
                    calls,
                    Options::with(|opt| {
                        opt.gas = Some(gas);
                        opt.nonce = Some(U256::from(nonce));
                    }),
                    KeyPair::from(signer),
                ))
                .map_err(|e| {
                    pink_extension::error!(
                        "Failed to submit step execution tx with error: {:?}",
                        e
                    );
                    "FailedToSubmitTransaction"
                })?;

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
                    .map_err(|e| {
                        pink_extension::error!(
                            "Failed to construct substrate tx with error: {:?}",
                            e
                        );
                        "FailedToCreateTransaction"
                    })?;

                    send_transaction(&chain.endpoint, &signed_tx).map_err(|e| {
                        pink_extension::error!(
                            "Failed to submit step execution tx with error: {:?}",
                            e
                        );
                        "FailedToSubmitTransaction"
                    })?
                }
                _ => return Err("UnexpectedCallType"),
            },
        };

        pink_extension::info!("Submitted step execution tx: {:?}", hex::encode(&tx_id));
        Ok(tx_id)
    }

    // By checking the nonce we can known whether the transaction has been executed or not,
    // and with help of off-chain indexer, we can get the relevant transaction's execution result.
    fn check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        pink_extension::info!(
            "Trying to check step execution result with nonce: {}",
            nonce
        );
        let as_single_step = self.as_single_step();
        let source_chain = as_single_step
            .source_chain(context)
            .ok_or("MissingSourceChain")?;
        let worker_account = AccountInfo::from(context.signer);
        let recipient = as_single_step.recipient.clone();

        // Query off-chain indexer directly get the execution result
        let account = match source_chain.chain_type {
            ChainType::Evm => worker_account.account20.to_vec(),
            ChainType::Sub => worker_account.account32.to_vec(),
        };
        if tx::check_tx(&source_chain.tx_indexer, &account, nonce)? {
            // If is a bridge operation, check balance change on dest chain
            if as_single_step.is_bridge_step() {
                pink_extension::info!(
                    "Check balance change on destchain for bridge step {:?}",
                    &as_single_step
                );
                let dest_chain = as_single_step
                    .dest_chain(context)
                    .ok_or("MissingSourceChain")?;
                let latest_balance =
                    dest_chain.get_balance(as_single_step.receive_asset.clone(), recipient)?;
                let origin_balance = as_single_step
                    .origin_balance
                    .ok_or("MissingOriginReserve")?;
                pink_extension::info!(
                    "origin_balance: {:?}, latest_balance: {:?}",
                    origin_balance,
                    latest_balance
                );

                return Ok(latest_balance > origin_balance);
            }
            return Ok(true);
        }
        Ok(false)
    }
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct StepSimulateResult {
    pub action_extra_info: ActionExtraInfo,
    // Estimate gas cost for the step on EVM chain
    pub gas_limit: Option<U256>,
    // Current suggest gas price on EVM chains
    pub gas_price: Option<U256>,
    // Native asset price in USD
    // the  USD amount is the value / 10000
    pub native_price_in_usd: u32,
    // tx fee will be paied in USD, calculated based on `gas_cost` and `gas_limit`
    // the USD amount is the value / 10000
    // potentially use Fixed crates here
    pub tx_fee_in_usd: u32,
}

pub trait Simulate {
    fn simulate(&mut self, context: &Context) -> Result<StepSimulateResult, &'static str>;
}

impl Simulate for MultiStep {
    #[allow(unused_variables)]
    fn simulate(&mut self, context: &Context) -> Result<StepSimulateResult, &'static str> {
        let action_extra_info = match self {
            MultiStep::Single(step) => context
                .get_action_extra_info(&step.source_chain, &step.exe)
                .ok_or("NoActionFound")?,
            MultiStep::Batch(batch_steps) => {
                let mut extra_info = ActionExtraInfo::default();
                for step in batch_steps.iter() {
                    let single_extra_step = context
                        .get_action_extra_info(&step.source_chain, &step.exe)
                        .ok_or("NoActionFound")?;
                    extra_info.extra_proto_fee_in_usd += single_extra_step.extra_proto_fee_in_usd;
                    extra_info.const_proto_fee_in_usd += single_extra_step.const_proto_fee_in_usd;
                    extra_info.percentage_proto_fee =
                        extra_info.percentage_proto_fee + single_extra_step.percentage_proto_fee;
                    // Batch txs will happen within same block, so we don't need to accumulate it
                    extra_info.confirm_time_in_sec = single_extra_step.confirm_time_in_sec;
                }
                extra_info
            }
        };

        // A minimal amount
        self.set_spend(1_000_000_000);
        let calls = self.derive_calls(context)?;

        let as_single_step = self.as_single_step();
        let chain = as_single_step
            .source_chain(context)
            .ok_or("MissingSourceChain")?;
        let worker_account = AccountInfo::from(context.signer);

        pink_extension::debug!("Start to simulate step with calls: {:?}", &calls);
        let (gas_limit, gas_price, native_price_in_usd, tx_fee_in_usd) = match chain.chain_type {
            ChainType::Evm => {
                let eth = Eth::new(PinkHttp::new(chain.endpoint));
                let handler = Contract::from_json(
                    eth.clone(),
                    chain.handler_contract.to_array().into(),
                    include_bytes!("./abi/handler.json"),
                )
                .expect("Bad abi data");
                let options = if as_single_step.spend_asset == chain.native_asset {
                    Options::with(|opt| {
                        opt.value = Some(U256::from(as_single_step.spend_amount.unwrap()))
                    })
                } else {
                    Options::default()
                };

                // Estiamte gas before submission
                let gas = resolve_ready(handler.estimate_gas(
                    "batchCall",
                    calls.clone(),
                    worker_account.account20.into(),
                    options,
                ))
                .map_err(|e| {
                    pink_extension::error!("Failed to estimated step gas cost with error: {:?}", e);
                    "FailedToEstimateGas"
                })?;

                let gas_price = resolve_ready(eth.gas_price()).or(Err("FailedToGetGasPrice"))?;
                let native_asset_price = crate::price::get_price(&chain.name, &chain.native_asset)
                    .ok_or("MissingPriceData")?;
                (
                    Some(gas),
                    Some(gas_price),
                    native_asset_price,
                    // The usd value is the return amount / 10000
                    // TODO: here we presume the decimals of all EVM native asset is 18, but we should get it from asset info
                    ((gas * gas_price * native_asset_price)
                        / U256::from(U256::from(10).pow(U256::from(18))))
                    .try_into()
                    .expect("Tx fee overflow"),
                )
            }
            ChainType::Sub => match calls[0].params.clone() {
                CallParams::Sub(SubCall { calldata }) => {
                    let native_asset_price =
                        crate::price::get_price(&chain.name, &chain.native_asset)
                            .ok_or("MissingPriceData")?;
                    (
                        None,
                        None,
                        native_asset_price,
                        // TODO: estimate tx_fee according to calldata size
                        // 0.001 USD
                        100,
                    )
                }
                _ => return Err("UnexpectedCallType"),
            },
        };

        Ok(StepSimulateResult {
            action_extra_info,
            gas_limit,
            gas_price,
            native_price_in_usd,
            tx_fee_in_usd,
        })
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
}
