use super::account::AccountInfo;
use super::context::Context;
use super::traits::Runner;
use alloc::vec::Vec;
use index_registry::types::{ChainInfo, ChainType};
use scale::{Decode, Encode};

/// Definition of bridge operation step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct BridgeStep {
    /// Asset id on source chain
    from: Vec<u8>,
    /// Name of source chain
    source_chain: Vec<u8>,
    /// Asset on dest chain
    to: Vec<u8>,
    /// Name of dest chain
    dest_chain: Vec<u8>,
    /// Fee of the bridge represented by the transfer asset
    fee: u128,
    /// Capacity of the step
    cap: u128,
    /// Flow of the step
    flow: u128,
    /// Original relayer account balance of asset on source chain
    /// Should be set when initializing task
    b0: Option<u128>,
    /// Original relayer account balance of asset on dest chain
    /// Should be set when initializing task
    b1: Option<u128>,
    /// Bridge amount
    amount: u128,
}

impl Runner for BridgeStep {
    fn run(&self, context: &Context) -> Result<(), &'static str> {
        let signer = context.signer;

        // Get executor according to `src_chain` and `des_chain`
        // let executor = context
        //     .registry
        //     .bridge_executors
        //     .get(&[self.source_chain, self.dest_chain].concat())
        //     .ok_or(Err("MissingExecutor"))?;
        // let chain = context
        //     .registry
        //     .chains
        //     .get(&self.dest_chain)
        //     .ok_or(Err("MissingChain"))?;
        // let recipient = match chain.chain_type {
        //     ChainType::Evm => AccountInfo::from(signer).account20,
        //     ChainType::Sub => AccountInfo::from(signer).account32,
        // };
        // // Do bridge transfer operation
        // executor
        //     .transfer(signer, self.from, recipient, self.amount)
        //     .map_err(|| Err("BridgeFailed"))?;
        Ok(())
    }

    fn check(&self, nonce: u64) -> bool {
        false
    }
}
