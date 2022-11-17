#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use super::common::Address;
use super::common::Amount;
use super::common::Error;
use alloc::vec::Vec;
use primitive_types::H256;
use scale::{Decode, Encode};

pub trait Executor {
    fn new(
        bridge_address: Address,
        abi_json: &[u8],
        rpc: &str,
    ) -> core::result::Result<Self, Error>
    where
        Self: Sized;
    fn transfer(
        &self,
        signer: [u8; 32], // FIXME
        token_rid: H256,
        amount: Amount,
        recipient: Address,
    ) -> core::result::Result<(), Error>;
}

/// Definition of source edge
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
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
    /// Original relayer account balance of spend asset
    b0: Option<u128>,
    /// Original relayer account balance of received asset
    b1: Option<u128>,
}

/// Definition of bridge operation edge
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
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
    /// Original relayer account balance of asset on source chain
    b0: Option<u128>,
    /// Original relayer account balance of asset on dest chain
    b1: Option<u128>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
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

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
pub enum EdgeMeta {
    Source(SourceEdge),
    Sink(SinkEdge),
    Swap(SwapEdge),
    Bridge(BridgeEdge),
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
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

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo,))]
pub struct Solution {
    /// All edges to included in the solution
    edges: Vec<Edge>,

    /// Sender address on source chain
    sender: Vec<u8>,
    /// Recipient address on dest chain
    recipient: Vec<u8>,
}
