use super::account::AccountInfo;
use alloc::vec::Vec;
use index_registry::RegistryRef;

#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Context {
    pub signer: [u8; 32],
    pub registry: RegistryRef,
    pub worker_accounts: Vec<AccountInfo>,
}

impl Context {
    pub fn get_account(&self, account32: [u8; 32]) -> Option<AccountInfo> {
        self.worker_accounts
            .iter()
            .position(|a| a.account32 == account32)
            .map(|idx| self.worker_accounts[idx].clone())
    }
}
