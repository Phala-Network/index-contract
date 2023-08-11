use crate::chain::ChainType;
use alloc::vec;
use alloc::{string::String, vec::Vec};

use crate::account::AccountInfo;
use crate::context::Context;
use crate::storage::StorageClient;
use crate::traits::Runner;
use crate::tx;
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
        })
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
        let _signer = context.signer;
        let worker_account = AccountInfo::from(context.signer);
        let chain = context
            .registry
            .get_chain(self.source_chain.clone())
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;

        let action = context
            .get_actions(self.source_chain.clone())
            .ok_or("NoActionFound")?;
        let call = action.build_call(self.clone())?;

        pink_extension::debug!("Start to execute step with nonce: {}", nonce);

        // TODO: Sign and send transaction
        let tx_id = match chain.chain_type {
            ChainType::Evm => {
                worker_account.account20.to_vec();
                vec![]
            }
            ChainType::Sub => {
                worker_account.account32.to_vec();
                vec![]
            }
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
