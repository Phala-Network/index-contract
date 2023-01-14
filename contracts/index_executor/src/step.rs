use super::bridge::BridgeStep;
use super::claimer::ClaimStep;
use super::context::Context;
use super::swap::SwapStep;
use super::traits::Runner;
use alloc::string::String;
use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;
use scale::{Decode, Encode};

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum StepMeta {
    Claim(ClaimStep),
    Swap(SwapStep),
    Bridge(BridgeStep),
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Step {
    /// Content of the step
    pub meta: StepMeta,
    /// Executing chain
    pub chain: String,
    /// Nonce of the worker account that related to this step execution
    pub nonce: Option<u64>,
}

impl Runner for Step {
    fn runnable(&self, client: &mut SubstrateRollupClient) -> bool {
        self.nonce.is_some()
            && match &self.meta {
                StepMeta::Claim(claim_step) => claim_step.runnable(client),
                StepMeta::Swap(swap_step) => swap_step.runnable(client),
                StepMeta::Bridge(bridge_step) => bridge_step.runnable(client),
            }
    }

    fn run(&self, nonce: u64, context: &Context) -> Result<(), &'static str> {
        match &self.meta {
            StepMeta::Claim(claim_step) => claim_step.run(nonce, context),
            StepMeta::Swap(swap_step) => swap_step.run(nonce, context),
            StepMeta::Bridge(bridge_step) => bridge_step.run(nonce, context),
        }
    }

    fn check(&self, _nonce: u64, context: &Context) -> bool {
        match &self.meta {
            StepMeta::Claim(claim_step) => claim_step.check(self.nonce.unwrap(), context),
            StepMeta::Swap(swap_step) => swap_step.check(self.nonce.unwrap(), context),
            StepMeta::Bridge(bridge_step) => bridge_step.check(self.nonce.unwrap(), context),
        }
    }
}
