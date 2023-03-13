pub mod ethereum_to_phala;
pub mod moonbeam_to_acala;
pub mod moonbeam_to_phala;
pub mod moonbeam_xtoken;
pub mod phala_to_acala;
pub mod phala_to_ethereum;
pub mod phala_xtransfer;

#[cfg(test)]
mod tests {
    use primitive_types::H256;
    use scale::Encode;
    use xcm::v1::{prelude::*, Junction, Junctions, MultiLocation};

    #[test]
    fn it_works() {
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
