use alloc::vec::Vec;
use scale::{Decode, Encode};

/// Definition of source edge
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SourceEdge {
    /// asset/chain
    pub to: Vec<u8>,
}

/// Definition of SINK edge
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SinkEdge {
    /// asset/chain
    pub from: Vec<u8>,
}

/// Definition of swap operation edge
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SwapEdge {
    /// asset/chain
    pub from: Vec<u8>,
    /// asset/chain
    pub to: Vec<u8>,
    /// Dex name
    pub dex: Vec<u8>,
    /// The amount flows in this step
    pub in_amount: u128,
    /// estimation in the form of range
    pub out_min: u128,
    pub out_max: u128,
}

/// Definition of bridge operation edge
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct BridgeEdge {
    /// asset/chain
    pub from: Vec<u8>,
    /// asset/chain
    pub to: Vec<u8>,
    pub in_amount: u128,
    /// estimation in the form of range
    pub out_min: u128,
    pub out_max: u128,
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct TransferEdge {
    /// asset/chain
    pub from: Vec<u8>,
    /// asset/chain
    pub to: Vec<u8>,
    pub in_amount: u128,
    pub out_min: u128,
    pub out_max: u128,
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum EdgeStatus {
    /// Haven't started executing this edge yet, which is the default status.
    Inactive,
    /// Transaction has been sent with transaction hash returned.
    Activated(Vec<u8>),
    /// Transaction has been sent but was dropped accidentally by the node.
    Dropped,
    /// Transaction has been sent but failed to execute by the node.
    Failed(Vec<u8>),
    /// Transaction has been sent and included in a specific block
    Confirmed(u128),
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum EdgeMeta {
    Source(SourceEdge),
    Sink(SinkEdge),
    Swap(SwapEdge),
    Bridge(BridgeEdge),
    Transfer(TransferEdge),
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Edge {
    /// Content of the edge
    pub edge: EdgeMeta,
    /// Status of the edge, updated by executor
    pub status: EdgeStatus,
    /// Distributed relayer account for this edge
    pub relayer: Option<Vec<u8>>,
    /// Public key of the relayer
    pub key: Option<[u8; 32]>,
    /// Nonce of the relayer on source chain of edge
    pub nonce: Option<u128>,
}

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum TaskStatus {
    /// Task initial confirmed by user on source chain.
    Initialized,
    /// Task is being claimed by worker. [tx_hash]
    Claiming(Option<Vec<u8>>),
    /// Task is being uploaded to on-chain storage. [tx_hash]
    Uploading(Option<Vec<u8>>),
    /// Task is being executing with step index. [step_index, tx_hash]
    Executing(u8, Option<Vec<u8>>),
    /// Task is being reverting with step index. [step_index, tx_hash]
    Reverting(u8, Option<Vec<u8>>),
    /// Last step of task has been executed successful last step on dest chain.
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
    /// All edges to included in the task
    pub edges: Vec<Edge>,
    /// Sender address on source chain
    pub sender: Vec<u8>,
    /// Recipient address on dest chain
    pub recipient: Vec<u8>,
}
