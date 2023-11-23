use sp_runtime::Permill;

pub mod acala;
pub mod astar;
pub mod base;
pub mod ethereum;
pub mod moonbeam;
pub mod phala;
pub mod polkadot;

#[derive(Clone, Debug, Default, scale::Decode, scale::Encode, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ActionExtraInfo {
    // The fee is a constant amount that will NOT deducted from user spend asset or receive asset
    // That means our worker will pay for this during execution, so it should be treat like tx fee
    // that should be deducted from user spend separately
    // Represent USD: extra_proto_fee_in_usd / 10000
    // We can potentially use Fixed crates here
    pub extra_proto_fee_in_usd: u32,
    // The fee is a constant amount that will deducted from user spend asset or receive asset
    // Represent USD: const_proto_fee_in_usd / 10000
    // We can potentially use Fixed crates here
    pub const_proto_fee_in_usd: u32,
    // The fee that calculated by a percentage scale, will deducted from user spend asset or receive asset
    pub percentage_proto_fee: Permill,
    pub confirm_time_in_sec: u16,
}
