use index_registry::RegistryRef;

#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Context {
    pub signer: [u8; 32],
    pub registry: RegistryRef,
}
