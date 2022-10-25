#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use alloc::vec::Vec;
use ink_lang as ink;
use ink_storage::traits::{
    PackedAllocate, PackedLayout, SpreadAllocate, SpreadLayout, StorageLayout,
};
use scale::{Decode, Encode};
use xcm::latest::{AssetId, MultiLocation};

/// Definition of source edge
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    SpreadLayout,
    PackedLayout,
    SpreadAllocate,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct SourceEdge {
    /// asset/chain
    to: Vec<u8>,
    /// Capacity of the edge
    cap: u128,
    /// Flow of the edge
    flow: u128,
    /// Price impact after executing the edge
    impact: u128,
}

/// Definition of SINK edge
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    SpreadLayout,
    PackedLayout,
    SpreadAllocate,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct SinkEdge {
    /// asset/chain
    from: Vec<u8>,
    /// Capacity of the edge
    cap: u128,
    /// Flow of the edge
    flow: u128,
    /// Price impact after executing the edge
    impact: u128,
}

/// Definition of swap operation edge
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    SpreadLayout,
    PackedLayout,
    SpreadAllocate,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct SwapEdge {
    /// asset/chain
    from: Vec<u8>,
    /// asset/chain
    to: Vec<u8>,
    /// Chain name
    chain: Vec<u8>,
    /// Dex name
    dex: Vec<u8>,
    /// Capacity of the edge
    cap: u128,
    /// Flow of the edge
    flow: u128,
    /// Price impact after executing the edge
    impact: u128,
}

/// Definition of bridge operation edge
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    SpreadLayout,
    PackedLayout,
    SpreadAllocate,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct BridgeEdge {
    /// asset/chain
    from: Vec<u8>,
    /// asset/chain
    to: Vec<u8>,
    /// Capacity of the edge
    cap: u128,
    /// Flow of the edge
    flow: u128,
    /// Price impact after executing the edge
    impact: u128,
}

#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
enum EdgeStatus {
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

#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode, SpreadLayout, PackedLayout)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub enum EdgeMeta {
    SourceEdge(SourceEdge),
    SinkEdge(SinkEdge),
    SwapEdge(SwapEdge),
    BridgeEdge(BridgeEdge),
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    SpreadLayout,
    PackedLayout,
    SpreadAllocate,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct Edge {
    /// Content of the edge
    edge: EdgeMeta,
    /// Status of the edge, updated by executor
    status: EdgeStatus,
    /// Distributed relayer account for this edge
    relayer: Option<Vec<u8>>,
    /// Public key of the relayer
    key: Option<[u8; 32]>,
    /// Nonce of the relayer on source chain of edge
    nonce: Option<u128>,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    SpreadLayout,
    PackedLayout,
    SpreadAllocate,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct Solution {
    /// All edges to included in the solution
    edges: Vec<Edge>,

    /// Sender address on source chain
    sender: Vec<u8>,
    /// Recipient address on dest chain
    recipient: Vec<u8>,
}
