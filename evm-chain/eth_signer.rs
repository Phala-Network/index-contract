#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use alloc::vec::Vec;
use scale::{Decode, Encode};
use traits::registry::{SignedTransaction, Signer};

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct EthSigner {
    /// Private key
    key: [u8; 32],
}

impl EthSigner {
    fn new(&mut self, key: [u8; 32]) -> Self {
        EthSigner { key }
    }
}

impl Signer for EthSigner {
    fn sign_transaction(&self, unsigned_tx: Vec<u8>) -> SignedTransaction {
        // TODO: return real signed transaction data
        SignedTransaction::EthSignedTransaction
    }
}
