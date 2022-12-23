pub mod bridge_executor;
pub mod dex_executor;

#[cfg(test)]
mod tests {

    use primitive_types::H256;
    use scale::Encode;
    use xcm::v0::NetworkId;
    use xcm::v1::{Junction, Junctions, MultiLocation};
    #[test]
    fn xcm_works() {
        use hex_literal::hex;
        let recipient: Vec<u8> =
            hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48").into();
        let addr: H256 = H256::from_slice(&recipient);
        let dest = MultiLocation::new(
            0,
            Junctions::X1(Junction::AccountId32 {
                network: NetworkId::Any,
                id: addr.into(),
            }),
        );
        let expected: Vec<u8> =
            hex!("000101008eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48").into();
        assert_eq!(dest.encode(), expected);
    }
}
