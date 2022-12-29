#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Dex {
    pub id: u32,
    pub name: String,
    pub chain_id: u32,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct DexIndexer {
    pub id: u32,
    pub url: String,
    pub dex_id: u32,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct DexPair {
    pub id: u32,
    pub asset0_id: u32,
    pub asset1_id: u32,
    pub dex_id: u32,
    pub pair_id: Vec<u8>,
}
