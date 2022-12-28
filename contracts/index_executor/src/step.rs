use super::bridge::BridgeStep;
use super::claimer::ClaimStep;
use super::context::Context;
use super::swap::SwapStep;
use super::traits::Runner;
use alloc::string::String;
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
    fn runnable(&self) -> bool {
        match &self.meta {
            StepMeta::Claim(claim_step) => claim_step.runnable(),
            StepMeta::Swap(swap_step) => swap_step.runnable(),
            StepMeta::Bridge(bridge_step) => bridge_step.runnable(),
        }
    }

    fn run(&self, context: &Context) -> Result<(), &'static str> {
        match &self.meta {
            StepMeta::Claim(claim_step) => claim_step.run(context),
            StepMeta::Swap(swap_step) => swap_step.run(context),
            StepMeta::Bridge(bridge_step) => bridge_step.run(context),
        }
    }

    fn check(&self, _nonce: u64) -> bool {
        match &self.meta {
            StepMeta::Claim(claim_step) => claim_step.check(self.nonce.unwrap()),
            StepMeta::Swap(swap_step) => swap_step.check(self.nonce.unwrap()),
            StepMeta::Bridge(bridge_step) => bridge_step.check(self.nonce.unwrap()),
        }
    }
}
