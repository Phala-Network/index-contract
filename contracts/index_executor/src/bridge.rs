use super::account::AccountInfo;
use super::context::Context;
use super::traits::Runner;
use alloc::{string::String, vec::Vec};
use index::graph::ChainType;
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
    fn runnable(&self) -> bool {
        // TODO: implement
        true
    }

    fn run(&self, context: &Context) -> Result<(), &'static str> {
        let signer = context.signer;

        // Get executor according to `src_chain` and `des_chain`
        let executor = context
            .get_bridge_executor(self.source_chain.clone(), self.dest_chain.clone())
            .ok_or("MissingExecutor")?;
        let chain = context
            .graph
            .get_chain(self.dest_chain.clone())
            .map(Ok)
            .unwrap_or(Err("MissingChain"))?;
        let recipient = match chain.chain_type {
            ChainType::Evm => AccountInfo::from(signer).account20.into(),
            ChainType::Sub => AccountInfo::from(signer).account32.into(),
        };
        // Do bridge transfer operation
        executor
            .transfer(signer, self.from.clone(), recipient, self.amount)
            .map_err(|_| "BridgeFailed")?;
        Ok(())
    }

    fn check(&self, _nonce: u64) -> bool {
        // TODO: implement
        false
    }
}
