use super::account::AccountInfo;
use super::context::Context;
use super::traits::Runner;
use alloc::vec::Vec;
use index_registry::types::{ChainInfo, ChainType};
use scale::{Decode, Encode};

/// Definition of swap operation step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SwapStep {
    /// Asset to spend
    pub send_asset: Vec<u8>,
    /// Asset to receive
    pub receive_asset: Vec<u8>,
    /// Chain name
    pub chain: Vec<u8>,
    /// Dex name
    pub dex: Vec<u8>,
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
    fn run(&self, context: &Context) -> Result<(), &'static str> {
        let signer = context.signer;

        // Get executor according to `chain` from registry
        // let executor = context
        //     .registry
        //     .dex_executors
        //     .get(&self.chain)
        //     .ok_or(Err("MissingExecutor"))?;
        // let source_chain = self
        //     .registry
        //     .chains
        //     .get(self.chain)
        //     .ok_or(Err("MissingChain"))?;
        // let recipient = match source_chain.chain_type {
        //     ChainType::Evm => AccountInfo::from(signer).account20,
        //     ChainType::Sub => AccountInfo::from(signer).account32,
        // };
        // // Do swap operation
        // let _ = executor
        //     .swap(
        //         signer,
        //         self.spend_asset,
        //         self.receive_asset,
        //         self.spend,
        //         recipient,
        //     )
        //     .map_err(|| Err("SwapFailed"))?;
        Ok(())
    }

    fn check(&self, nonce: u64) -> bool {
        false
    }
}
