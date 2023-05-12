use alloc::string::String;

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Dex {
    pub id: u32,
    pub name: String,
    pub chain_id: u32,
    pub indexer: String,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct DexPair {
    pub id: u32,
    pub asset0_id: u32,
    pub asset1_id: u32,
    pub dex_id: u32,
    // not always prefixed with 0x!
    pub pair_id: String,
}
