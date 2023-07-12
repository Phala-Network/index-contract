use super::account::AccountInfo;
use crate::graph::Graph;
use alloc::{boxed::Box, string::String, vec::Vec};
use index::{prelude::*, traits::executor::TransferExecutor};

pub struct Context {
    pub signer: [u8; 32],
    pub graph: Graph,
    pub worker_accounts: Vec<AccountInfo>,
    /// (source_chain, dest_chain) => bridge_executor
    pub bridge_executors: Vec<((String, String), Box<dyn BridgeExecutor>)>,
    /// source_chain => dex_executor
    pub dex_executors: Vec<(String, Box<dyn DexExecutor>)>,
    /// source_chain => transfer_executor
    pub transfer_executors: Vec<(String, Box<dyn TransferExecutor>)>,
}

impl Context {
    pub fn get_account(&self, account32: [u8; 32]) -> Option<AccountInfo> {
        self.worker_accounts
            .iter()
            .position(|a| a.account32 == account32)
            .map(|idx| self.worker_accounts[idx].clone())
    }

    pub fn get_bridge_executor(
        &self,
        source_chain: String,
        dest_chain: String,
    ) -> Option<Box<dyn BridgeExecutor>> {
        pink_extension::debug!(
            "Lookup bridge executor between {:?} <> {:?}",
            &source_chain,
            &dest_chain,
        );
        self.bridge_executors
            .iter()
            .position(|e| e.0 .0 == source_chain && e.0 .1 == dest_chain)
            .map(|idx| dyn_clone::clone_box(&*self.bridge_executors[idx].1))
    }

    pub fn get_dex_executor(&self, source_chain: String) -> Option<Box<dyn DexExecutor>> {
        pink_extension::debug!("Lookup dex executor on {:?}", &source_chain);
        self.dex_executors
            .iter()
            .position(|e| e.0 == source_chain)
            .map(|idx| dyn_clone::clone_box(&*self.dex_executors[idx].1))
    }

    pub fn get_transfer_executor(&self, chain: String) -> Option<Box<dyn TransferExecutor>> {
        pink_extension::debug!("Lookup transfer executor on {:?}", &chain);
        self.transfer_executors
            .iter()
            .position(|e| e.0 == chain)
            .map(|idx| dyn_clone::clone_box(&*self.transfer_executors[idx].1))
    }
}
