use super::account::AccountInfo;
use crate::registry::Registry;
use alloc::{boxed::Box, string::String, vec::Vec};
use index::{prelude::*, traits::executor::TransferExecutor};

pub struct Context<'a> {
    pub signer: [u8; 32],
    pub registry: &'a Registry,
    pub worker_accounts: Vec<AccountInfo>,
}

impl<'a> Context<'a> {
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
        let bridge_executors = self.registry.create_bridge_executors();

        bridge_executors
            .iter()
            .position(|e| e.0 .0 == source_chain && e.0 .1 == dest_chain)
            .map(|idx| dyn_clone::clone_box(&*bridge_executors[idx].1))
    }

    pub fn get_dex_executor(&self, source_chain: String) -> Option<Box<dyn DexExecutor>> {
        pink_extension::debug!("Lookup dex executor on {:?}", &source_chain);
        let dex_executors = self.registry.create_dex_executors();
        dex_executors
            .iter()
            .position(|e| e.0 == source_chain)
            .map(|idx| dyn_clone::clone_box(&*dex_executors[idx].1))
    }

    pub fn get_transfer_executor(&self, chain: String) -> Option<Box<dyn TransferExecutor>> {
        pink_extension::debug!("Lookup transfer executor on {:?}", &chain);
        let transfer_executors: Vec<(String, Box<dyn TransferExecutor>)> =
            self.registry.create_transfer_executors();
        transfer_executors
            .iter()
            .position(|e| e.0 == chain)
            .map(|idx| dyn_clone::clone_box(&*transfer_executors[idx].1))
    }
}
