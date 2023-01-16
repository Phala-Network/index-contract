use super::account::AccountInfo;
use super::context::Context;
use super::traits::Runner;
use alloc::{string::String, vec::Vec};
use index::graph::ChainType;
use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;
use pink_subrpc::ExtraParam;
use scale::{Decode, Encode};

/// Definition of bridge operation step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct BridgeStep {
    /// Asset id on source chain
    pub from: Vec<u8>,
    /// Name of source chain
    pub source_chain: String,
    /// Asset on dest chain
    pub to: Vec<u8>,
    /// Name of dest chain
    pub dest_chain: String,
    /// Fee of the bridge represented by the transfer asset
    pub fee: u128,
    /// Capacity of the step
    pub cap: u128,
    /// Flow of the step
    pub flow: u128,
    /// Original relayer account balance of asset on source chain
    /// Should be set when initializing task
    pub b0: Option<u128>,
    /// Original relayer account balance of asset on dest chain
    /// Should be set when initializing task
    pub b1: Option<u128>,
    /// Bridge amount
    pub amount: u128,
}

impl Runner for BridgeStep {
    // The way we check if a bridge task is available to run is by:
    //
    // first by checking the nonce of the worker account, if the account nonce on source chain is great than
    // the nonce we apply to the step, that means the transaction revalant to the step already been executed.
    // In this situation we return false.
    //
    // second by checking the `spend_asset` balance of the worker account on the source chain, if the balance is
    // great than or equal to the `spend`, we think we can safely execute swap transaction
    fn runnable(
        &self,
        nonce: u64,
        context: &Context,
        _client: Option<&mut SubstrateRollupClient>,
    ) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);

        // 1. Check nonce
        let onchain_nonce = worker_account.get_nonce(self.source_chain.clone(), context)?;
        if onchain_nonce > nonce {
            return Err("StepAlreadyExecuted");
        }
        // 2. Check balance
        let onchain_balance =
            worker_account.get_balance(self.source_chain.clone(), self.from.clone(), context)?;
        Ok(onchain_balance >= self.amount)
    }

    fn run(&self, nonce: u64, context: &Context) -> Result<(), &'static str> {
        let signer = context.signer;

        // Get executor according to `src_chain` and `des_chain`
        let executor = context
            .get_bridge_executor(self.source_chain.clone(), self.dest_chain.clone())
            .ok_or("MissingExecutor")?;
        let chain = context
            .graph
            .get_chain(self.dest_chain.clone())
            .ok_or("MissingChain")?;
        let recipient = match chain.chain_type {
            ChainType::Evm => AccountInfo::from(signer).account20.into(),
            ChainType::Sub => AccountInfo::from(signer).account32.into(),
        };
        // Do bridge transfer operation
        executor
            .transfer(
                signer,
                self.from.clone(),
                recipient,
                self.amount,
                ExtraParam {
                    tip: 0,
                    nonce: Some(nonce),
                    era: None,
                },
            )
            .map_err(|_| "BridgeFailed")?;
        Ok(())
    }

    // By checking the nonce we can known whether the transaction has been executed or not,
    // and with help of off-chain indexer, we can get the relevant transaction's execution result.
    fn check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);

        // TODO. query off-chain indexer directly get the execution result
        // Check nonce
        let onchain_nonce = worker_account.get_nonce(self.source_chain.clone(), context)?;
        if onchain_nonce <= nonce {
            return Ok(false);
        }

        // Check balance change on source chain
        let onchain_balance =
            worker_account.get_balance(self.source_chain.clone(), self.from.clone(), context)?;
        Ok((self.b0.unwrap() - onchain_balance) == self.amount)
    }

    fn sync_check(&self, _nonce: u64, _context: &Context) -> Result<bool, &'static str> {
        Ok(true)
        // TODO: implement
    }
}
