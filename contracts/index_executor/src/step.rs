use crate::chain::ChainType;
use alloc::vec;
use alloc::{string::String, vec::Vec};

use crate::account::AccountInfo;
use crate::context::Context;
use crate::storage::StorageClient;
use crate::traits::Runner;
use crate::tx;

pub struct Step {
    exe_type: String,
    exe: String,
    source_chain: String,
    dest_chain: String,
    spend_asset: Vec<u8>,
    receive_asset: Vec<u8>,
    sender: Vec<u8>,
    recipient: Vec<u8>,
    spend_amount: u128,
    // Used to check balance change
    origin_balance: Option<u128>,
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
            &hex::encode(&self.sender),
            &self.source_chain,
            &self.dest_chain,
            &hex::encode(&self.recipient),
            self.spend_amount,
            hex::encode(&tx_id)
        );
        Ok(tx_id)
    }

    // By checking the nonce we can known whether the transaction has been executed or not,
    // and with help of off-chain indexer, we can get the relevant transaction's execution result.
    fn check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);

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
                let latest_balance = worker_account.get_balance(
                    self.dest_chain.clone(),
                    self.recipient.clone(),
                    context,
                )?;
                let origin_balance = self.origin_balance.ok_or("MissingOriginReserve")?;

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
