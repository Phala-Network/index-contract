use super::context::Context;
use super::storage::StorageClient;
use alloc::vec::Vec;
use xcm::v3::MultiLocation;


pub trait Runner {
    /// Check if a job can be executed.
    /// If the transaction already sent to blockchain, e.g. can be found in memory pool,
    /// it should be `unrunable`.
    /// If the transaction failed to execute, it should be `unrunable`.
    /// Else the job should be `runnable`.
    fn can_run(
        &self,
        nonce: u64,
        context: &Context,
        client: Option<&StorageClient>,
    ) -> Result<bool, &'static str>;

    /// Execute a job, basically send a transaction to blockchain, and return tx id.
    fn run(&mut self, nonce: u64, context: &Context) -> Result<Vec<u8>, &'static str>;

    /// Check if a job is already executed successfully when executing the job.
    ///
    /// Only when the transaction was successfully executed, it can return `true`
    fn has_finished(&self, nonce: u64, context: &Context) -> Result<bool, &'static str>;
}

pub trait AssetRegistry<T> {
    fn get_assetid(&self, chain: &str, location: &MultiLocation) -> Option<T>;

    fn get_location(&self, chain: &str, asset_id: T) -> Option<MultiLocation>;
}
