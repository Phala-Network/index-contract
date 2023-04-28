use crate::account::AccountInfo;
use crate::context::Context;
use crate::traits::Runner;
use alloc::{string::String, vec::Vec};
use index::tx;
use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;
use pink_subrpc::ExtraParam;
use scale::{Decode, Encode};

use super::ExtraResult;

/// Definition of bridge operation step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct BridgeStep {
    pub name: String,
    /// Asset id on source chain
    pub from: Vec<u8>,
    /// Name of source chain
    pub source_chain: String,
    /// Asset on dest chain
    pub to: Vec<u8>,
    /// Name of dest chain
    pub dest_chain: String,
    /// Recipient account on dest chain
    pub recipient: Option<Vec<u8>>,
    /// Actual amount of token0
    pub spend: u128,
    /// Reception in the form of range
    pub receive_min: u128,
    pub receive_max: u128,
    // the block info before this step is executed
    pub block_number: u64,
    pub index_in_block: u64,
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

        let src_indexer = &context
            .graph
            .get_chain(self.source_chain.clone())
            .ok_or("MissingChain")?
            .tx_indexer;

        let dest_indexer = &context
            .graph
            .get_chain(self.dest_chain.clone())
            .ok_or("MissingChain")?
            .tx_indexer;

        let chain = &context
            .graph
            .get_chain(self.source_chain.clone())
            .ok_or("MissingChain")?;

        let src_account = worker_account.account20.to_vec();
        let dest_account = worker_account.account32.to_vec();

        pink_extension::debug!(
            "runnable: trying: account: {}/{}, src_indexer: {}, dest_indexer: {}, src_nonce: {}, step: {:?}",
            hex::encode(&src_account),
            hex::encode(&dest_account),
            src_indexer,
            dest_indexer,
            nonce,
            self
        );

        let tx_result = tx::is_bridge_tx_ok(
            &src_account,
            &dest_account,
            src_indexer,
            nonce,
            dest_indexer,
            self.receive_min,
            self.receive_max,
            self.block_number,
            self.index_in_block,
        )
        .or(Err("Can't confirm transaction"))?;

        // if tx is ok then the step is not runnable
        Ok(!tx_result.0)
    }

    fn run(&self, nonce: u64, context: &Context) -> Result<Vec<u8>, &'static str> {
        let signer = context.signer;
        let recipient = self.recipient.clone().ok_or("MissingRecipient")?;

        pink_extension::debug!("Start to run bridge with nonce: {}", nonce);
        // Get executor according to `src_chain` and `des_chain`
        let executor = context
            .get_bridge_executor(self.source_chain.clone(), self.dest_chain.clone())
            .ok_or("MissingExecutor")?;
        pink_extension::debug!("Found bridge executor on {:?}", &self.source_chain);

        // Do bridge transfer operation
        let tx_id = executor
            .transfer(
                signer,
                self.from.clone(),
                recipient.clone(),
                self.spend,
                ExtraParam {
                    tip: 0,
                    nonce: Some(nonce),
                    era: None,
                },
            )
            .map_err(|_| "BridgeFailed")?;
        pink_extension::debug!(
            "Submit transaction to bridge asset {:?} from {:?} to {:?}, recipient: {:?}, amount: {:?}, tx id: {:?}",
            &hex::encode(&self.from),
            &self.source_chain,
            &self.dest_chain,
            &hex::encode(&recipient),
            self.spend,
            hex::encode(&tx_id)
        );
        Ok(tx_id)
    }

    // By checking the nonce we can known whether the transaction has been executed or not,
    // and with help of off-chain indexer, we can get the relevant transaction's execution result.
    fn check(&self, nonce: u64, context: &Context) -> Result<(bool, ExtraResult), &'static str> {
        let worker_account = AccountInfo::from(context.signer);
        let src_indexer = &context
            .graph
            .get_chain(self.source_chain.clone())
            .ok_or("MissingChain")?
            .tx_indexer;

        let dest_indexer = &context
            .graph
            .get_chain(self.dest_chain.clone())
            .ok_or("MissingChain")?
            .tx_indexer;

        let chain = &context
            .graph
            .get_chain(self.source_chain.clone())
            .ok_or("MissingChain")?;

        let account = match chain.chain_type {
            index::graph::ChainType::Evm => worker_account.account20.to_vec(),
            index::graph::ChainType::Sub => worker_account.account32.to_vec(),
        };
        let src_account = worker_account.account20.to_vec();
        let dest_account = worker_account.account32.to_vec();

        pink_extension::debug!(
            "runnable: trying: account: {}/{}, src_indexer: {}, dest_indexer: {}, src_nonce: {}, step: {:?}",
            hex::encode(&src_account),
            hex::encode(&dest_account),
            src_indexer,
            dest_indexer,
            nonce,
            self
        );

        let tx_result = tx::is_bridge_tx_ok(
            &src_account,
            &dest_account,
            src_indexer,
            nonce,
            dest_indexer,
            self.receive_min,
            self.receive_max,
            self.block_number,
            self.index_in_block,
        )
        .or(Err("Can't confirm transaction"))?;

        Ok((tx_result.0, ExtraResult::BlockInfo(tx_result.1)))
    }

    fn sync_check(
        &self,
        nonce: u64,
        context: &Context,
    ) -> Result<(bool, ExtraResult), &'static str> {
        pink_extension::debug!("Bridge step sync checking: {:?}", self);
        self.check(nonce, context)
    }
}
