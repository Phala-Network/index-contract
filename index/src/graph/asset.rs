#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Asset {
    pub id: u32,
    pub symbol: String,
    pub name: String,
    // of type MultiLocation
    pub location: Vec<u8>,
    pub decimals: u32,
    pub chain_id: u32,
}
