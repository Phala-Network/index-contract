use super::context::Context;
use crate::chain::{BalanceFetcher, ChainType, NonceFetcher};
use alloc::{format, string::String, vec::Vec};
use index::utils::ToArray;
use ink::storage::traits::StorageLayout;
use pink_extension::chain_extension::{signing, SigType};
use pink_extension::ResultExt;
use scale::{Decode, Encode};

#[derive(Clone)]
pub enum AccountType {
    Account20,
    Account32,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout,))]
pub struct AccountInfo {
    pub account32: [u8; 32],
    pub account20: [u8; 20],
}

impl AccountInfo {
    /// returns account raw bytes
    pub fn get_raw_account(
        &self,
        chain_name: String,
        context: &Context,
    ) -> Result<Vec<u8>, &'static str> {
        let chain = context
            .registry
            .get_chain(chain_name)
            .ok_or("MissingChain")?;
        Ok(match chain.chain_type {
            ChainType::Evm => self.account20.into(),
            ChainType::Sub => self.account32.into(),
        })
    }

    pub fn get_balance(
        &self,
        chain_name: String,
        asset: Vec<u8>,
        context: &Context,
    ) -> Result<u128, &'static str> {
        let chain = context
            .registry
            .get_chain(chain_name)
            .ok_or("MissingChain")?;
        let account: Vec<u8> = match chain.chain_type {
            ChainType::Evm => self.account20.into(),
            ChainType::Sub => self.account32.into(),
        };
        chain
            .get_balance(asset, account)
            .map_err(|_| "FetchBalanceFailed")
    }

    pub fn get_nonce(&self, chain_name: String, context: &Context) -> Result<u64, &'static str> {
        let chain = context
            .registry
            .get_chain(chain_name.clone())
            .ok_or("MissingChain")?;
        let account: Vec<u8> = match chain.chain_type {
            ChainType::Evm => self.account20.into(),
            ChainType::Sub => self.account32.into(),
        };
        chain
            .get_nonce(account.clone())
            .log_err(&format!(
                "Fetch nonce failed, chain: {:?}, account: {:?}",
                &chain_name,
                &hex::encode(&account)
            ))
            .map_err(|_| "FetchNonceFailed")
    }
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
