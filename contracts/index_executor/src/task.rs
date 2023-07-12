use super::account::AccountInfo;
use super::context::Context;
use super::traits::Runner;
use crate::chain::{ChainType, NonceFetcher};
use crate::steps::{Step, StepMeta};
use crate::storage::StorageClient;
use alloc::{string::String, vec, vec::Vec};
use ink::storage::Mapping;
use scale::{Decode, Encode};

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum TaskStatus {
    /// Task initial confirmed by user on source chain.
    Actived,
    /// Task has been initialized, e.g. being applied nonce.
    Initialized,
    /// Task is being executing with step index.
    /// Transaction can be indentified by worker account nonce on specific chain
    /// [step_index, worker_nonce]
    Executing(u8, Option<u64>),
    /// Last step of task has been executed successfully on dest chain.
    Completed,
}

pub type TaskId = [u8; 32];

#[derive(Clone, Decode, Encode, Eq, PartialEq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Task {
    // Task id
    pub id: TaskId,
    // Allocated worker account public key to execute the task
    pub worker: [u8; 32],
    // Task status
    pub status: TaskStatus,
    // Source chain name
    pub source: String,
    /// All steps to included in the task
    pub steps: Vec<Step>,
    /// Current step index that is executing
    pub execute_index: u8,
    /// Sender address on source chain
    pub sender: Vec<u8>,
    /// Recipient address on dest chain
    pub recipient: Vec<u8>,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: [0; 32],
            worker: [0; 32],
            status: TaskStatus::Actived,
            source: String::default(),
            steps: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: vec![],
        }
    }
}

impl Task {
    // Initialize task
    pub fn init_and_submit(
        &mut self,
        context: &Context,
        client: &StorageClient,
    ) -> Result<(), &'static str> {
        let mut free_accounts = client.lookup_free_accounts().ok_or("WorkerAccountNotSet")?;
        let mut pending_tasks = client.lookup_pending_tasks();

        if client.lookup_task(&self.id).is_some() {
            // Task already saved, return
            return Ok(());
        }

        // Lookup free worker list to find if the worker we expected is free, if it's free remove it or return error
        if let Some(index) = free_accounts.iter().position(|&x| x == self.worker) {
            free_accounts.remove(index);
            pink_extension::debug!(
                "Worker {:?} is free, will be applied to this task {:?}.",
                hex::encode(self.worker),
                hex::encode(self.id)
            );
        } else {
            pink_extension::debug!(
                "Worker {:?} is busy, try again later for this task {:?}.",
                hex::encode(self.worker),
                hex::encode(self.id)
            );
            return Err("WorkerIsBusy");
        }

        // Apply worker nonce for each step in task
        self.apply_nonce(context, client)?;
        // Apply recipient for each step in task
        self.apply_recipient(context)?;
        // TODO: query initial balance of worker account and setup to specific step
        self.status = TaskStatus::Initialized;
        self.execute_index = 0;
        // Push to pending tasks queue
        pending_tasks.push(self.id);
        // Save task data
        client.put(self.id.as_ref(), &self.encode())?;

        client.put(b"free_accounts".as_ref(), &free_accounts.encode())?;
        client.put(b"pending_tasks".as_ref(), &pending_tasks.encode())?;
        Ok(())
    }

    pub fn execute(
        &mut self,
        context: &Context,
        client: &StorageClient,
    ) -> Result<TaskStatus, &'static str> {
        // To avoid unnecessary remote check, we check index in advance
        if self.execute_index as usize == self.steps.len() {
            return Ok(TaskStatus::Completed);
        }

        // If step already executed successfully, execute next step
        if self.steps[self.execute_index as usize].check(
            // An executing task must have nonce applied
            self.steps[self.execute_index as usize].nonce.unwrap(),
            context,
        ) == Ok(true)
        // FIXME: handle returned error
        {
            self.execute_index += 1;
            // If all step executed successfully, set task as `Completed`
            if self.execute_index as usize == self.steps.len() {
                self.status = TaskStatus::Completed;
                return Ok(self.status.clone());
            }

            // Settle before execute next step
            let settle_balance = self.settle(context)?;
            pink_extension::debug!(
                "Settle balance of last step[{:?}], settle amount: {:?}",
                (self.execute_index - 1),
                settle_balance,
            );
            // Update balance that actually can be consumed
            self.update_balance(settle_balance, context)?;
            pink_extension::debug!("Finished previous step execution");

            // An executing task must have nonce applied
            let nonce = self.steps[self.execute_index as usize].nonce.unwrap();
            // FIXME: handle returned error
            if self.steps[self.execute_index as usize].runnable(nonce, context, Some(client))
                == Ok(true)
            {
                pink_extension::debug!(
                    "Trying to run step[{:?}] with nonce {:?}",
                    self.execute_index,
                    nonce
                );
                self.steps[self.execute_index as usize].run(nonce, context)?;
                self.status = TaskStatus::Executing(self.execute_index, Some(nonce));
            } else {
                pink_extension::debug!("Step[{:?}] not runnable, return", self.execute_index);
            }
        } else {
            let nonce = self.steps[self.execute_index as usize].nonce.unwrap();
            // Claim step should be considered separately
            if let StepMeta::Claim(claim_step) = &mut self.steps[self.execute_index as usize].meta {
                let worker_account = AccountInfo::from(context.signer);
                let latest_balance = worker_account.get_balance(
                    claim_step.chain.clone(),
                    claim_step.asset.clone(),
                    context,
                )?;
                claim_step.b0 = Some(latest_balance);

                // FIXME: handle returned error
                if self.steps[self.execute_index as usize].runnable(nonce, context, Some(client))
                    == Ok(true)
                {
                    pink_extension::debug!("Trying to claim task with nonce {:?}", nonce);
                    self.steps[self.execute_index as usize].run(nonce, context)?;
                    self.status = TaskStatus::Executing(self.execute_index, Some(nonce));
                } else {
                    pink_extension::debug!("Claim step not runnable, return");
                }
            }
        }
        Ok(self.status.clone())
    }

    /// Delete task record from on-chain storage
    pub fn destroy(&mut self, client: &StorageClient) -> Result<(), &'static str> {
        let mut free_accounts = client.lookup_free_accounts().ok_or("WorkerAccountNotSet")?;
        let mut pending_tasks = client.lookup_pending_tasks();

        if client.lookup_task(&self.id).is_some() {
            if let Some(idx) = pending_tasks.iter().position(|id| *id == self.id) {
                // Remove from pending tasks queue
                pending_tasks.remove(idx);
                // Recycle worker account
                free_accounts.push(self.worker);
                // Delete task data
                client.delete(self.id.as_ref())?;
            }
            client.put(b"free_accounts".as_ref(), &free_accounts.encode())?;
            client.put(b"pending_tasks".as_ref(), &pending_tasks.encode())?;
        }

        Ok(())
    }

    fn apply_nonce(
        &mut self,
        context: &Context,
        _client: &StorageClient,
    ) -> Result<(), &'static str> {
        // Only in last step the recipient with be set as real recipient on destchain,
        // or else it will be the worker account under the hood
        let mut nonce_map: Mapping<String, u64> = Mapping::default();
        for step in self.steps.iter_mut() {
            let nonce = match nonce_map.get(&step.chain) {
                Some(nonce) => nonce,
                None => {
                    let chain = context
                        .registry
                        .get_chain(step.chain.clone())
                        .ok_or("MissingChain")?;
                    let account_info = context.get_account(self.worker).ok_or("WorkerNotFound")?;
                    let account = match chain.chain_type {
                        ChainType::Evm => account_info.account20.to_vec(),
                        ChainType::Sub => account_info.account32.to_vec(),
                        // ChainType::Unknown => panic!("chain not supported!"),
                    };
                    chain.get_nonce(account).map_err(|_| "FetchNonceFailed")?
                }
            };
            step.nonce = Some(nonce);
            // Increase nonce by 1
            nonce_map.insert(step.chain.clone(), &(nonce + 1));
        }

        Ok(())
    }

    fn apply_recipient(&mut self, context: &Context) -> Result<(), &'static str> {
        let step_count = self.steps.len();
        for (index, step) in self.steps.iter_mut().enumerate() {
            match &mut step.meta {
                StepMeta::Swap(swap_step) => {
                    swap_step.recipient = if index == (step_count - 1) {
                        Some(self.recipient.clone())
                    } else {
                        let chain = context
                            .registry
                            .get_chain(swap_step.chain.clone())
                            .ok_or("MissingChain")?;
                        let account_info =
                            context.get_account(self.worker).ok_or("WorkerNotFound")?;
                        Some(match chain.chain_type {
                            ChainType::Evm => account_info.account20.to_vec(),
                            ChainType::Sub => account_info.account32.to_vec(),
                            // ChainType::Unknown => panic!("chain not supported!"),
                        })
                    };
                }
                StepMeta::Bridge(bridge_step) => {
                    bridge_step.recipient = if index == (step_count - 1) {
                        Some(self.recipient.clone())
                    } else {
                        let chain = context
                            .registry
                            .get_chain(bridge_step.dest_chain.clone())
                            .ok_or("MissingChain")?;
                        let account_info =
                            context.get_account(self.worker).ok_or("WorkerNotFound")?;
                        Some(match chain.chain_type {
                            ChainType::Evm => account_info.account20.to_vec(),
                            ChainType::Sub => account_info.account32.to_vec(),
                            // ChainType::Unknown => panic!("chain not supported!"),
                        })
                    };
                }
                StepMeta::Transfer(transfer_step) => {
                    transfer_step.recipient = if index == (step_count - 1) {
                        Some(self.recipient.clone())
                    } else {
                        let chain = context
                            .registry
                            .get_chain(transfer_step.chain.clone())
                            .ok_or("MissingChain")?;
                        let account_info =
                            context.get_account(self.worker).ok_or("WorkerNotFound")?;
                        Some(match chain.chain_type {
                            ChainType::Evm => account_info.account20.to_vec(),
                            ChainType::Sub => account_info.account32.to_vec(),
                            // ChainType::Unknown => panic!("chain not supported!"),
                        })
                    };
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn settle(&mut self, context: &Context) -> Result<u128, &'static str> {
        if self.execute_index < 1 {
            return Err("InvalidExecuteIndex");
        }

        let last_step = self.steps[(self.execute_index - 1) as usize].clone();
        let worker_account = AccountInfo::from(context.signer);
        Ok(match last_step.meta {
            StepMeta::Claim(claim_step) => {
                let old_balance = claim_step.b0.ok_or("MisingOriginBalance")?;
                let latest_balance =
                    worker_account.get_balance(claim_step.chain, claim_step.asset, context)?;
                // FIXME: what if some bad guy transfer this asset into worker account
                latest_balance.saturating_sub(old_balance)
            }
            StepMeta::Swap(swap_step) => {
                let old_balance = swap_step.b1.ok_or("MisingOriginBalance")?;
                let latest_balance = worker_account.get_balance(
                    swap_step.chain,
                    swap_step.receive_asset,
                    context,
                )?;
                latest_balance.saturating_sub(old_balance)
            }
            StepMeta::Bridge(bridge_step) => {
                // Old balance on dest chain
                let old_balance = bridge_step.b1.ok_or("MisingOriginBalance")?;
                let latest_balance =
                    worker_account.get_balance(bridge_step.dest_chain, bridge_step.to, context)?;
                latest_balance.saturating_sub(old_balance)
            }
            StepMeta::Transfer(transfer_step) => {
                // Old balance on dest chain
                let old_balance = transfer_step.b1.ok_or("MisingOriginBalance")?;
                let latest_balance = worker_account.get_balance(
                    transfer_step.chain,
                    transfer_step.asset,
                    context,
                )?;
                latest_balance.saturating_sub(old_balance)
            }
        })
    }

    fn update_balance(
        &mut self,
        settle_balance: u128,
        context: &Context,
    ) -> Result<(), &'static str> {
        if self.execute_index < 1 {
            return Err("InvalidExecuteIndex");
        }
        let worker_account = AccountInfo::from(context.signer);
        match &mut self.steps[self.execute_index as usize].meta {
            StepMeta::Swap(swap_step) => {
                swap_step.spend = settle_balance.min(swap_step.flow);

                // Update the original balance of worker account
                let latest_b0 = worker_account.get_balance(
                    swap_step.chain.clone(),
                    swap_step.spend_asset.clone(),
                    context,
                )?;
                let latest_b1 = worker_account.get_balance(
                    swap_step.chain.clone(),
                    swap_step.receive_asset.clone(),
                    context,
                )?;
                swap_step.b0 = Some(latest_b0);
                swap_step.b1 = Some(latest_b1);
            }
            StepMeta::Bridge(bridge_step) => {
                bridge_step.amount = settle_balance.min(bridge_step.flow);

                // Update bridge asset the original balance of worker account
                let latest_b0 = worker_account.get_balance(
                    bridge_step.source_chain.clone(),
                    bridge_step.from.clone(),
                    context,
                )?;
                let latest_b1 = worker_account.get_balance(
                    bridge_step.dest_chain.clone(),
                    bridge_step.to.clone(),
                    context,
                )?;
                bridge_step.b0 = Some(latest_b0);
                bridge_step.b1 = Some(latest_b1);
            }
            StepMeta::Transfer(transfer_step) => {
                transfer_step.amount = settle_balance.min(transfer_step.flow);

                // sender's balance
                let latest_b0 = worker_account.get_balance(
                    transfer_step.chain.clone(),
                    transfer_step.asset.clone(),
                    context,
                )?;
                // recipeint's balance
                let latest_b1 = worker_account.get_balance(
                    transfer_step.chain.clone(),
                    transfer_step.asset.clone(),
                    context,
                )?;
                transfer_step.b0 = Some(latest_b0);
                transfer_step.b1 = Some(latest_b1);
            }
            _ => return Err("UnexpectedStep"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountInfo;
    use crate::chain::{Chain, ChainType};
    use crate::registry::Registry;
    use crate::steps::claimer::ActivedTaskFetcher;
    use dotenv::dotenv;
    use hex_literal::hex;
    use pink_extension::chain_extension::AccountId;
    use primitive_types::H160;

    #[ink::test]
    fn test_get_evm_account_nonce() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();
        let _ = env_logger::try_init();

        let goerli = Chain {
            id: 1,
            name: String::from("Goerli"),
            endpoint: String::from(
                "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
            ),
            chain_type: ChainType::Evm,
            native_asset: vec![0],
            foreign_asset: None,
            handler_contract: "0x056C0E37d026f9639313C281250cA932C9dbe921".into(),
            tx_indexer_url: Default::default(),
        };
        assert_eq!(
            goerli
                .get_nonce(hex!("0E275F8839b788B2674935AD97C01cF73A9E8c41").into())
                .unwrap(),
            2
        );
    }

    #[ignore]
    #[ink::test]
    fn task_init_should_work() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();
        // Secret key of test account `//Alice`
        let sk_alice = hex!("e5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a");
        // Prepare executor account
        let executor_key =
            pink_web3::keys::pink::KeyPair::derive_keypair(b"executor").private_key();
        let executor_pub: [u8; 32] = pink_extension::ext()
            .get_public_key(
                pink_extension::chain_extension::SigType::Sr25519,
                &executor_key,
            )
            .try_into()
            .unwrap();
        let contract_id: AccountId = executor_pub.into();

        // Prepare worker accounts
        let mut worker_accounts: Vec<AccountInfo> = vec![];
        for index in 0..10 {
            let private_key = pink_web3::keys::pink::KeyPair::derive_keypair(
                &[b"worker".to_vec(), [index].to_vec()].concat(),
            )
            .private_key();
            worker_accounts.push(AccountInfo::from(private_key));
        }

        // Create storage client
        let client: StorageClient = StorageClient::new("url".to_string(), "key".to_string());
        // Setup initial worker accounts to storage
        client
            .set_worker_accounts(
                worker_accounts
                    .clone()
                    .into_iter()
                    .map(|account| account.account32.clone())
                    .collect(),
            )
            .unwrap();

        // Fetch actived task from chain
        let pre_mock_executor_address: H160 =
            hex!("f60dB2d02af3f650798b59CB6D453b78f2C1BC90").into();
        let mut task = ActivedTaskFetcher {
            chain: Chain {
                id: 0,
                name: String::from("Ethereum"),
                chain_type: ChainType::Evm,
                endpoint: String::from(
                    "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
                ),
                native_asset: vec![0],
                foreign_asset: None,
                handler_contract: "0x056C0E37d026f9639313C281250cA932C9dbe921".into(),
                tx_indexer_url: Default::default(),
            },
            worker: AccountInfo {
                account20: pre_mock_executor_address.into(),
                account32: [0; 32],
            },
        }
        .fetch_task()
        .unwrap()
        .unwrap();
        assert_eq!(task.steps.len(), 3);

        // Init task
        assert_eq!(task.init_and_submit(
            &Context {
                signer: [0; 32],
                registry: &Registry {
                    chains: vec![
                        Chain {
                            id: 1,
                            name: String::from("Khala"),
                            endpoint: String::from("http://127.0.0.1:39933"),
                            chain_type: ChainType::Sub,
                            native_asset: vec![0],
                            foreign_asset: None,
                            handler_contract: "0x056C0E37d026f9639313C281250cA932C9dbe921".into(),
                            tx_indexer_url: Default::default(),
                        },
                        Chain {
                            id: 2,
                            name: String::from("Ethereum"),
                            endpoint: String::from("https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2"),
                            chain_type: ChainType::Evm,
                            native_asset: vec![0],
                            foreign_asset: None,
                            handler_contract: "0x056C0E37d026f9639313C281250cA932C9dbe921".into(),
                            tx_indexer_url: Default::default(),
                        }
                    ],
                },
                worker_accounts: worker_accounts.clone(),
            },
            &client,
        ), Ok(()));

        // Now let's query if the task is exist in rollup storage with another rollup client
        let another_client = StorageClient::new("another url".to_string(), "key".to_string());
        let onchain_task = another_client.lookup_task(&task.id).unwrap();
        assert_eq!(onchain_task.status, TaskStatus::Initialized);
        assert_eq!(
            onchain_task.worker,
            worker_accounts.last().unwrap().account32
        );
        assert_eq!(onchain_task.steps.len(), 3);
        assert_eq!(onchain_task.steps[0].nonce, Some(0));
        assert_eq!(onchain_task.steps[1].nonce, Some(1));
        assert_eq!(onchain_task.steps[2].nonce, Some(2));
    }
}
