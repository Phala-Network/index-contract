use super::account::AccountInfo;
use super::context::Context;
use super::traits::Runner;
use alloc::{string::String, vec::Vec};
use index::graph::ChainType;
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
}

impl Runner for SwapStep {
    // The way we check if a bridge task is available to run is by:
    //
    // first by checking the nonce of the worker account, if the account nonce on source chain is great than
    // the nonce we apply to the step, that means the transaction revalant to the step already been executed.
    // In this situation we return false.
    //
    // second by checking the `from` asset balance of the worker account on the source chain, if the balance is
    // great than or equal to the `amount` to bridge, we think we can safely execute bridge transaction
    fn runnable(
        &self,
        nonce: u64,
        context: &Context,
        _client: Option<&mut SubstrateRollupClient>,
    ) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);

        // 1. Check nonce
        let onchain_nonce = worker_account.get_nonce(self.chain.clone(), context)?;
        if onchain_nonce > nonce {
            return Ok(false);
        }
        // 2. Check balance
        let onchain_balance =
            worker_account.get_balance(self.chain.clone(), self.spend_asset.clone(), context)?;
        Ok(onchain_balance >= self.spend)
    }

    fn run(
        &self,
        nonce: u64,
        recipient: Option<Vec<u8>>,
        context: &Context,
    ) -> Result<Vec<u8>, &'static str> {
        let signer = context.signer;

        pink_extension::debug!("Start to run swap with nonce: {}", nonce);
        // Get executor according to `chain` from registry
        let executor = context
            .get_dex_executor(self.chain.clone())
            .ok_or("MissingExecutor")?;
        pink_extension::debug!("Found dex executor on {:?}", &self.chain);
        let source_chain = context
            .graph
            .get_chain(self.chain.clone())
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;
        let recipient = recipient.unwrap_or(match source_chain.chain_type {
            ChainType::Evm => AccountInfo::from(signer).account20.into(),
            ChainType::Sub => AccountInfo::from(signer).account32.into(),
        });
        pink_extension::debug!(
            "Trying to swap assset to recipient: {:?}",
            hex::encode(&recipient)
        );

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
            &self.spend_asset,
            &self.receive_asset,
            &self.chain,
            &recipient,
            self.spend,
            hex::encode(&tx_id)
        );
        Ok(tx_id)
    }

    // By checking the nonce we can known whether the transaction has been executed or not,
    // and with help of off-chain indexer, we can get the relevant transaction's execution result.
    fn check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);

        // TODO. query off-chain indexer directly get the execution result
        // Check nonce
        let onchain_nonce = worker_account.get_nonce(self.chain.clone(), context)?;
        if onchain_nonce <= nonce {
            return Ok(false);
        }

        // Check balance change on source chain
        let onchain_balance =
            worker_account.get_balance(self.chain.clone(), self.spend_asset.clone(), context)?;
        Ok((self.b0.unwrap() - onchain_balance) == self.spend)
    }

    fn sync_check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        self.check(nonce, context)
    }
}
