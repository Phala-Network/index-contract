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
    fn runnable(
        &self,
        context: &Context,
        _client: Option<&mut SubstrateRollupClient>,
    ) -> Result<bool, &'static str> {
        // TODO: implement
        Ok(true)
    }

    fn run(&self, nonce: u64, context: &Context) -> Result<(), &'static str> {
        let signer = context.signer;

        // Get executor according to `chain` from registry
        let executor = context
            .get_dex_executor(self.chain.clone())
            .ok_or("MissingExecutor")?;
        let source_chain = context
            .graph
            .get_chain(self.chain.clone())
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;
        let recipient = match source_chain.chain_type {
            ChainType::Evm => AccountInfo::from(signer).account20.into(),
            ChainType::Sub => AccountInfo::from(signer).account32.into(),
        };
        // Do swap operation
        let _ = executor
            .swap(
                signer,
                self.spend_asset.clone(),
                self.receive_asset.clone(),
                self.spend,
                recipient,
                ExtraParam {
                    tip: 0,
                    nonce: Some(nonce),
                    era: None,
                },
            )
            .map_err(|_| "SwapFailed")?;
        Ok(())
    }

    fn check(&self, _nonce: u64, _context: &Context) -> Result<bool, &'static str> {
        // TODO: implement
        Ok(false)
    }
}
