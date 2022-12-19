use alloc::vec::Vec;
use scale::{Decode, Encode};

/// Definition of source step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct BeginStep {
    /// asset/chain
    pub to: Vec<u8>,
    /// Capacity of the step
    pub cap: u128,
    /// Flow of the step
    pub flow: u128,
    /// Price impact after executing the step
    pub impact: u128,
}

/// Definition of SINK step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct EndStep {
    /// asset/chain
    pub from: Vec<u8>,
    /// Capacity of the step
    pub cap: u128,
    /// Flow of the step
    pub flow: u128,
    /// Price impact after executing the step
    pub impact: u128,
}

/// Definition of swap operation step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SwapStep {
    /// asset/chain
    pub from: Vec<u8>,
    /// asset/chain
    pub to: Vec<u8>,
    /// Chain name
    pub chain: Vec<u8>,
    /// Dex name
    pub dex: Vec<u8>,
    /// Capacity of the step
    pub cap: u128,
    /// Flow of the step
    pub flow: u128,
    /// Price impact after executing the step
    pub impact: u128,
    /// Original relayer account balance of spend asset
    pub b0: Option<u128>,
    /// Original relayer account balance of received asset
    pub b1: Option<u128>,
}

/// Definition of bridge operation step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct BridgeStep {
    /// asset/chain
    from: Vec<u8>,
    /// asset/chain
    to: Vec<u8>,
    /// Capacity of the step
    cap: u128,
    /// Flow of the step
    flow: u128,
    /// Price impact after executing the step
    impact: u128,
    /// Original relayer account balance of asset on source chain
    b0: Option<u128>,
    /// Original relayer account balance of asset on dest chain
    b1: Option<u128>,
}

/// Definition of bridge operation step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ClaimStep {
    /// Chain name
    chain: Vec<u8>,
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum StepMeta {
    Claim(ClaimStep),
    Begin(BeginStep),
    Swap(SwapStep),
    Bridge(BridgeStep),
    End(EndStep),
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Step {
    /// Content of the step
    pub step: StepMeta,
    /// Nonce of the worker account that related to this step execution
    pub nonce: Option<u128>,
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum TaskStatus {
    /// Task initial confirmed by user on source chain.
    Initialized,
    /// Task is being executing with step index.
    /// Transaction can be indentified by worker account nonce on specific chain
    /// [step_index, worker_nonce]
    Executing(u8, Option<u64>),
    /// Last step of task has been executed successfully on dest chain.
    Completed,
}

pub type TaskId = [u8; 32];

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Task {
    // Task id
    pub id: TaskId,
    // Allocated worker account public key to execute the task
    pub worker: [u8; 32],
    // Task status
    pub status: TaskStatus,
    // Source chain name
    pub source: Vec<u8>,
    /// All steps to included in the task
    pub steps: Vec<Step>,
    /// Sender address on source chain
    pub sender: Vec<u8>,
    /// Recipient address on dest chain
    pub recipient: Vec<u8>,
}
