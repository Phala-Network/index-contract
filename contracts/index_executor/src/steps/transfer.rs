use crate::account::AccountInfo;
use crate::context::Context;
use crate::traits::Runner;
use alloc::{string::String, vec::Vec};
use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;
use pink_subrpc::ExtraParam;
use scale::{Decode, Encode};

use super::ExtraResult;

/// Definition of swap operation step
#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct TransferStep {
    // Asset to be transferred
    pub asset: Vec<u8>,
    pub amount: u128,
    pub chain: String,
    /// Actual amount of token0
    pub spend: u128,
    /// Reception in the form of range
    pub receive_min: u128,
    pub receive_max: u128,
    // Recipient account on current chain
    pub recipient: Option<Vec<u8>>,
}

impl Runner for TransferStep {
    fn runnable(
        &self,
        nonce: u64,
        context: &Context,
        _client: Option<&mut SubstrateRollupClient>,
    ) -> Result<bool, &'static str> {
        let worker_account = AccountInfo::from(context.signer);

        // 1. Check nonce
        let onchain_nonce = worker_account.get_nonce(self.chain.clone(), context)?;
        if onchain_nonce > nonce {
            return Ok(false);
        }
        // 2. Check balance
        let onchain_balance =
            worker_account.get_balance(self.chain.clone(), self.asset.clone(), context)?;
        Ok(onchain_balance >= self.amount)
    }

    fn run(&self, nonce: u64, context: &Context) -> Result<Vec<u8>, &'static str> {
        let signer = context.signer;
        let recipient = self.recipient.clone().ok_or("MissingRecipient")?;

        pink_extension::debug!("Start to tranfer with nonce: {}", nonce);
        // Get executor according to `chain` from registry
        let executor = context
            .get_transfer_executor(self.chain.clone())
            .ok_or("MissingExecutor")?;
        pink_extension::debug!("Found transfer executor on {:?}", &self.chain);

        let tx_id = executor
            .transfer(
                signer,
                self.asset.clone(),
                recipient.clone(),
                self.amount,
                ExtraParam {
                    tip: 0,
                    nonce: Some(nonce),
                    era: None,
                },
            )
            .map_err(|_| "SwapFailed")?;
        pink_extension::info!(
            "Submit transaction to transfer asset {:?} on ${:?}, recipient: {:?}, amount: {:?}, tx id: {:?}",
            &hex::encode(&self.asset),
            &self.chain,
            &hex::encode(&recipient),
            self.amount,
            hex::encode(&tx_id)
        );
        Ok(tx_id)
    }

    /// nonce: from the current state, haven't synced with the onchain state,
    ///     must be smaller than that of the current state if the last step succeeded
    fn check(&self, nonce: u64, context: &Context) -> Result<(bool, ExtraResult), &'static str> {
        let worker = AccountInfo::from(context.signer);
        // Check nonce
        let onchain_nonce = worker.get_nonce(self.chain.clone(), context)?;
        if onchain_nonce <= nonce {
            return Ok((false, ExtraResult::None));
        }
        Ok((true, ExtraResult::None))
    }

    fn sync_check(
        &self,
        nonce: u64,
        context: &Context,
    ) -> Result<(bool, ExtraResult), &'static str> {
        self.check(nonce, context)
    }
}
