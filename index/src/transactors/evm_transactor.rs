use crate::traits::common::Error;
use crate::utils::ToBeBytes;
use pink_web3::contract::{Contract, Options};
use pink_web3::ethabi::{Bytes, Uint};
use pink_web3::keys::pink::KeyPair;
use pink_web3::signing::Key;
use pink_web3::transports::{resolve_ready, PinkHttp};
use pink_web3::types::H256;

/// The client to submit transaction to the Evm evm_contract contract
#[derive(Clone)]
pub struct ChainBridgeDepositer {
    pub dest_chain_id: u8,
    pub contract: Contract<PinkHttp>,
}

impl ChainBridgeDepositer {
    /// Calls the EVM contract `deposit` function
    ///
    /// # Arguments
    ///
    /// * `dest_chain_id` - ID of chain deposit originated from.
    /// * `token_rid` - resource id used to find address of token handler to be used for deposit
    /// * `data` - Addition data to be passed to special handler
    pub fn deposit(
        &self,
        signer: KeyPair,
        token_rid: H256,
        amount: Uint,
        recipient_address: Bytes,
        nonce: Option<u64>,
    ) -> core::result::Result<H256, Error> {
        let data = Self::compose_deposite_data(amount, recipient_address);
        let params = (self.dest_chain_id, token_rid, data);
        // Estiamte gas before submission
        let gas = resolve_ready(self.contract.estimate_gas(
            "deposit",
            params.clone(),
            signer.address(),
            Options::default(),
        ))
        .map_err(|_| Error::FailedToGetGas)?;

        // Actually submit the tx (no guarantee for success)
        let tx_id = resolve_ready(self.contract.signed_call(
            "deposit",
            params,
            Options::with(|opt| {
                opt.gas = Some(gas);
                opt.nonce = nonce.map(|nonce| nonce.into());
            }),
            signer,
        ))
        .map_err(|_| Error::FailedToSubmitTransaction)?;
        Ok(tx_id)
    }

    /// Composes the `data` argument to the chainbridge `deposit` function
    ///
    /// The signature of the solidity `deposit` function is as follows:
    ///
    /// function deposit(uint8 destinationChainID,
    ///     bytes32 resourceID,
    ///     bytes calldata data)
    /// external payable whenNotPaused;
    ///  
    /// `Data` passed into the function should be constructed as follows:
    /// * `amount`                      uint256     bytes   0 - 32
    /// * `recipientAddress length`     uint256     bytes  32 - 64
    /// * `recipientAddress`            bytes       bytes  64 - END
    fn compose_deposite_data(amount: Uint, recipient_address: Bytes) -> Bytes {
        let ra_len = recipient_address.len();
        [
            amount.to_be_fixed_bytes(),
            ra_len.to_be_fixed_bytes(),
            recipient_address,
        ]
        .concat()
    }
}
