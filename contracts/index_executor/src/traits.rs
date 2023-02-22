use super::context::Context;
use alloc::vec::Vec;
use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;

pub trait Runner {
    /// Check if a job can be executed.
    /// If the transaction already sent to blockchain, e.g. can be found in memory pool,
    /// it should be `unrunable`.
    /// If the transaction failed to execute, it should be `unrunable`.
    /// Else the job should be `runnable`.
    fn runnable(
        &self,
        nonce: u64,
        context: &Context,
        client: Option<&mut SubstrateRollupClient>,
    ) -> Result<bool, &'static str>;

    /// Execute a job, basically send a transaction to blockchain, and return tx id.
    fn run(&self, nonce: u64, context: &Context) -> Result<Vec<u8>, &'static str>;

    /// Check if a job is already executed successfully when executing the job.
    ///
    /// Only when the transaction was successfully executed, it can return `true`
    fn check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str>;

    /// Check if a job is already executed successfully when sync (recover) from rollup.
    ///
    /// For bridge operation, not only the transaction was successfully executed on
    /// source chain, but also need to be executed on dest chain. We can not acquire
    /// enough information from phat contract, so to check result on dest chain, we
    /// must depend on the information of off-chain indexer
    fn sync_check(&self, nonce: u64, context: &Context) -> Result<bool, &'static str>;
}
