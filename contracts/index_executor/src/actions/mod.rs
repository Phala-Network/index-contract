use sp_runtime::Permill;

pub mod acala;
pub mod astar;
pub mod base;
pub mod ethereum;
pub mod moonbeam;
pub mod phala;
pub mod polkadot;

#[derive(Clone, scale::Decode, scale::Encode, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ActionExtraInfo {
    // Represent USD: const_proto_fee / 10000
    // We can potentially use Fixed crates here
    pub const_proto_fee: u16,
    pub percentage_proto_fee: Permill,
    pub confirm_time: u16,
}
