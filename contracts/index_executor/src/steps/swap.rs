use crate::account::AccountInfo;
use crate::context::Context;
use crate::traits::Runner;
use alloc::{string::String, vec::Vec};
use index::tx;
use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;
use pink_subrpc::ExtraParam;
use scale::{Decode, Encode};

/// Definition of swap operation step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SwapStep {
    /// Asset to spend
    pub spend_asset: Vec<u8>,
    /// Asset to receive
    pub receive_asset: Vec<u8>,
    /// Chain name
    pub chain: String,
    /// Dex name
    pub dex: String,
    /// Capacity of the step
    pub cap: u128,
    /// Flow of the step
    pub flow: u128,
    /// Price impact after executing the step
    pub impact: u128,
    /// Original relayer account balance of spend asset
    /// Should be set when initializing task
    pub b0: Option<u128>,
    /// Original relayer account balance of received asset
    /// Should be set when initializing task
    pub b1: Option<u128>,
    /// Amount to be spend
    pub spend: u128,
    /// Recipient account on current chain
    pub recipient: Option<Vec<u8>>,
}

impl Runner for SwapStep {
    // The way we check if a bridge task is available to run is by checking the nonce
    //      of the worker account, if the account nonce on source chain is great than
    // the nonce we apply to the step, that means the transaction revalant to the step already been executed.
    // In this situation we return false.
    fn runnable(
        &self,
        nonce: u64,
        context: &Context,
        _client: Option<&mut SubstrateRollupClient>,
    ) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);

        let chain = &context
            .graph
            .get_chain(self.chain.clone())
            .ok_or("MissingChain")?;
        let account = match chain.chain_type {
            index::graph::ChainType::Evm => worker_account.account20.to_vec(),
            index::graph::ChainType::Sub => worker_account.account32.to_vec(),
        };
        // if ok then not runnable
        Ok(!tx::is_tx_ok(&chain.tx_indexer, &account, nonce).or(Err("Indexer failure"))?)
    }

    fn run(&self, nonce: u64, context: &Context) -> Result<Vec<u8>, &'static str> {
        let signer = context.signer;
        let recipient = self.recipient.clone().ok_or("MissingRecipient")?;

        pink_extension::debug!("Start to run swap with nonce: {}", nonce);
        // Get executor according to `chain` from registry
        let executor = context
            .get_dex_executor(self.chain.clone())
            .ok_or("MissingExecutor")?;
        pink_extension::debug!("Found dex executor on {:?}", &self.chain);

        // Do swap operation
        let tx_id = executor
            .swap(
                signer,
                self.spend_asset.clone(),
                self.receive_asset.clone(),
                self.spend,
                recipient.clone(),
                ExtraParam {
                    tip: 0,
                    nonce: Some(nonce),
                    era: None,
                },
            )
            .map_err(|_| "SwapFailed")?;
        pink_extension::info!(
            "Submit transaction to swap asset {:?} to {:?} on ${:?}, recipient: {:?}, spend: {:?}, tx id: {:?}",
            &hex::encode(&self.spend_asset),
            &hex::encode(&self.receive_asset),
            &self.chain,
            &hex::encode(&recipient),
            self.spend,
            hex::encode(&tx_id)
        );
        Ok(tx_id)
    }

    // By checking the nonce we can known whether the transaction has been executed or not,
    // and with help of off-chain indexer, we can get the relevant transaction's execution result.
    fn check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);

        let chain = &context
            .graph
            .get_chain(self.chain.clone())
            .ok_or("MissingChain")?;
        let account = match chain.chain_type {
            index::graph::ChainType::Evm => worker_account.account20.to_vec(),
            index::graph::ChainType::Sub => worker_account.account32.to_vec(),
        };
        tx::is_tx_ok(&chain.tx_indexer, &account, nonce).or(Err("Indexer failure"))
    }

    fn sync_check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        self.check(nonce, context)
    }
}
