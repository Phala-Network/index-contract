#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Bridge {
    pub id: u32,
    pub name: String,
    pub location: Vec<u8>,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct BridgePair {
    pub id: u32,
    pub asset0_id: u32,
    pub asset1_id: u32,
    pub bridge_id: u32,
}

