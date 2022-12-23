use index::utils::ToArray;
use pink_extension::chain_extension::{signing, SigType};
use scale::{Decode, Encode};

#[derive(Encode, Decode, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct AccountInfo {
    pub account32: [u8; 32],
    pub account20: [u8; 20],
}

impl From<[u8; 32]> for AccountInfo {
    fn from(privkey: [u8; 32]) -> Self {
        let ecdsa_pubkey: [u8; 33] = signing::get_public_key(&privkey, SigType::Ecdsa)
            .try_into()
            .expect("Public key should be of length 33");
        let mut ecdsa_address = [0u8; 20];
        ink_env::ecdsa_to_eth_address(&ecdsa_pubkey, &mut ecdsa_address)
            .expect("Get address of ecdsa failed");
        Self {
            account32: signing::get_public_key(&privkey, SigType::Sr25519).to_array(),
            account20: ecdsa_address,
        }
    }
}
