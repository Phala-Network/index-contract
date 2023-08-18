use super::account::AccountInfo;
use crate::{call::CallBuilder, registry::Registry};
use alloc::{boxed::Box, string::String, vec::Vec};

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

    pub fn get_actions(&self, chain: &String, exe: &String) -> Option<Box<dyn CallBuilder>> {
        pink_extension::debug!("Lookup actions on {:?}", &chain);
        let actions: Vec<(String, Box<dyn CallBuilder>)> = self.registry.create_actions(&chain);
        actions
            .iter()
            .position(|e| e.0.to_lowercase() == exe.to_lowercase())
            .map(|idx| dyn_clone::clone_box(&*actions[idx].1))
    }
}
