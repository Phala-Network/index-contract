use super::context::Context;
use super::step::Step;
use super::traits::Runner;
use alloc::{string::String, vec, vec::Vec};
use index::graph::ChainType;
use index::graph::NonceFetcher;
use ink_storage::Mapping;
use kv_session::traits::KvSession;
use phat_offchain_rollup::clients::substrate::SubstrateRollupClient;
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
    pub fn init(&mut self, context: &Context, client: &mut SubstrateRollupClient) {
        let mut free_accounts = OnchainAccounts::lookup_free_accounts(client);
        let mut pending_tasks = OnchainTasks::lookup_pending_tasks(client);

        if OnchainTasks::lookup_task(client, &self.id).is_some() {
            // Task already saved, return
            return;
        }
        if let Some(account) = free_accounts.pop() {
            // Apply a worker account
            self.worker = account;
            // Aplly worker nonce for each step in task
            self.aplly_nonce(context, client);
            // TODO: query initial balance of worker account and setup to specific step
            self.status = TaskStatus::Initialized;
            self.execute_index = 0;
            // Push to pending tasks queue
            pending_tasks.push(self.id);
            // Save task data
            client.session().put(&self.id.to_vec(), self.encode());
        } else {
            // We can not handle more tasks any more
            return;
        }

        client
            .session()
            .put(&b"free_accounts".to_vec(), free_accounts.encode());
        client
            .session()
            .put(&b"pending_tasks".to_vec(), pending_tasks.encode());
    }

    // Recover execution status according to on-chain storage
    pub fn sync(&mut self, context: &Context, _client: &SubstrateRollupClient) {
        for step in self.steps.iter() {
            if step.sync_check(step.nonce.unwrap(), context) == Ok(true) {
                self.execute_index += 1;
                // If all step executed successfully, set task as `Completed`
                if self.execute_index as usize == self.steps.len() {
                    self.status = TaskStatus::Completed;
                    break;
                }
            } else {
                self.status = TaskStatus::Executing(self.execute_index, step.nonce);
                // Exit with current status
                break;
            }
        }
    }

    pub fn execute(
        &mut self,
        context: &Context,
        client: &mut SubstrateRollupClient,
    ) -> Result<TaskStatus, &'static str> {
        // If step already executed successfully, execute next step
        if self.steps[self.execute_index as usize].check(
            self.steps[self.execute_index as usize].nonce.unwrap(),
            context,
        ) == Ok(true)
        {
            self.execute_index += 1;
            // If all step executed successfully, set task as `Completed`
            if self.execute_index as usize == self.steps.len() {
                self.status = TaskStatus::Completed;
                return Ok(self.status.clone());
            }
        }

        let nonce = self.steps[self.execute_index as usize].nonce.unwrap();
        // FIXME: handle return error
        if self.steps[self.execute_index as usize].runnable(nonce, context, Some(client))
            == Ok(true)
        {
            self.steps[self.execute_index as usize].run(nonce, context)?;
            self.status = TaskStatus::Executing(self.execute_index, Some(nonce));
        }
        Ok(self.status.clone())
    }

    /// Delete task record from on-chain storage
    pub fn destroy(&mut self, client: &mut SubstrateRollupClient) {
        let mut free_accounts = OnchainAccounts::lookup_free_accounts(client);
        let mut pending_tasks = OnchainTasks::lookup_pending_tasks(client);

        if OnchainTasks::lookup_task(client, &self.id).is_some() {
            if let Some(idx) = pending_tasks.iter().position(|id| *id == self.id) {
                // Remove from pending tasks queue
                pending_tasks.remove(idx);
                // Recycle worker account
                free_accounts.push(self.worker);
                // Delete task data
                client.session().delete(&self.id.to_vec());
            }
            client
                .session()
                .put(&b"free_accounts".to_vec(), free_accounts.encode());
            client
                .session()
                .put(&b"pending_tasks".to_vec(), pending_tasks.encode());
        }
    }

    fn aplly_nonce(&mut self, context: &Context, _client: &SubstrateRollupClient) {
        let mut nonce_map: Mapping<String, u64> = Mapping::default();
        for step in self.steps.iter_mut() {
            let nonce = nonce_map.get(&step.chain).or_else(|| {
                let chain = context.graph.get_chain(step.chain.clone()).unwrap();
                let account_info = context.get_account(self.worker).unwrap();
                let account = match chain.chain_type {
                    ChainType::Evm => account_info.account20.to_vec(),
                    ChainType::Sub => account_info.account32.to_vec(),
                    // ChainType::Unknown => panic!("chain not supported!"),
                };
                chain.get_nonce(account).ok()
            });
            step.nonce = nonce;
            // Increase nonce by 1
            nonce_map.insert(step.chain.clone(), &(nonce.unwrap() + 1));
        }
    }
}

pub struct OnchainTasks;
impl OnchainTasks {
    pub fn lookup_task(client: &mut SubstrateRollupClient, id: &TaskId) -> Option<Task> {
        if let Ok(Some(raw_task)) = client.session().get(&id.to_vec()) {
            return match Decode::decode(&mut raw_task.as_slice()) {
                Ok(task) => Some(task),
                Err(_) => None,
            };
        }
        None
    }

    pub fn lookup_pending_tasks(client: &mut SubstrateRollupClient) -> Vec<TaskId> {
        if let Ok(Some(raw_tasks)) = client.session().get(&b"pending_tasks".to_vec()) {
            return match Decode::decode(&mut raw_tasks.as_slice()) {
                Ok(tasks) => tasks,
                Err(_) => vec![],
            };
        }
        vec![]
    }
}

pub struct OnchainAccounts;
impl OnchainAccounts {
    pub fn lookup_free_accounts(client: &mut SubstrateRollupClient) -> Vec<[u8; 32]> {
        if let Ok(Some(raw_accounts)) = client.session().get(&b"free_accounts".to_vec()) {
            return match Decode::decode(&mut raw_accounts.as_slice()) {
                Ok(free_accounts) => free_accounts,
                Err(_) => vec![],
            };
        }
        vec![]
    }

    pub fn set_worker_accounts(client: &mut SubstrateRollupClient, accounts: Vec<[u8; 32]>) {
        client
            .session()
            .put(&b"free_accounts".to_vec(), accounts.encode());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountInfo;
    use crate::claimer::ActivedTaskFetcher;
    use dotenv::dotenv;
    use hex_literal::hex;
    use index::graph::{Chain, ChainType, Graph};
    use ink_lang as ink;
    use phat_offchain_rollup::clients::substrate::{
        claim_name, get_name_owner, SubstrateRollupClient,
    };
    use primitive_types::H160;

    fn config_rollup(
        rollup_endpoint: String,
        contract_id: &ink_env::AccountId,
        submit_key: [u8; 32],
    ) -> Result<(), &'static str> {
        // Check if the rollup is initialized properly
        let actual_owner = get_name_owner(&rollup_endpoint, contract_id).unwrap();
        if let Some(owner) = actual_owner {
            let pubkey = pink_extension::ext().get_public_key(
                pink_extension::chain_extension::SigType::Sr25519,
                &submit_key,
            );
            if owner.encode() != pubkey {
                return Err("Slot owner dismatch");
            }
        } else {
            // Not initialized. Let's claim the name.
            claim_name(&rollup_endpoint, 100, &contract_id, &submit_key).unwrap();
        }
        Ok(())
    }

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
        let contract_id: ink_env::AccountId = executor_pub.into();

        // Prepare worker accounts
        let mut worker_accounts: Vec<AccountInfo> = vec![];
        for index in 0..10 {
            let private_key = pink_web3::keys::pink::KeyPair::derive_keypair(
                &[b"worker".to_vec(), [index].to_vec()].concat(),
            )
            .private_key();
            worker_accounts.push(AccountInfo::from(private_key));
        }

        // Config rollup, alice sent first transaction, nonce = 0
        assert_eq!(
            config_rollup(
                String::from("http://127.0.0.1:39933"),
                &contract_id,
                sk_alice
            ),
            Ok(())
        );

        // Create rollup client
        let mut client =
            SubstrateRollupClient::new("http://127.0.0.1:39933", 100, &contract_id).unwrap();
        // Setup initial worker accounts to rollup storage
        OnchainAccounts::set_worker_accounts(
            &mut client,
            worker_accounts
                .clone()
                .into_iter()
                .map(|account| account.account32.clone())
                .collect(),
        );

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
            },
            executor: AccountInfo {
                account20: pre_mock_executor_address.into(),
                account32: [0; 32],
            },
        }
        .fetch_task()
        .unwrap();
        assert_eq!(task.steps.len(), 3);

        // Init task
        task.init(
            &Context {
                signer: [0; 32],
                graph: Graph {
                    chains: vec![
                        Chain {
                            id: 1,
                            name: String::from("Khala"),
                            endpoint: String::from("http://127.0.0.1:39933"),
                            chain_type: ChainType::Sub,
                            native_asset: vec![0],
                            foreign_asset: None,
                        },
                        Chain {
                            id: 2,
                            name: String::from("Ethereum"),
                            endpoint: String::from("https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2"),
                            chain_type: ChainType::Evm,
                            native_asset: vec![0],
                            foreign_asset: None,
                        }
                    ],
                    assets: vec![],
                    dexs: vec![],
                    dex_pairs: vec![],
                    dex_indexers: vec![],
                    bridges: vec![],
                    bridge_pairs: vec![],
                },
                worker_accounts: worker_accounts.clone(),
                bridge_executors: vec![],
                dex_executors: vec![],
            },
            &mut client,
        );

        let maybe_submittable = client.commit().unwrap();
        if let Some(submittable) = maybe_submittable {
            let _tx_id = submittable.submit(&sk_alice, 1).unwrap();
        }

        // Wait 3 seconds
        std::thread::sleep(std::time::Duration::from_millis(3000));

        // Now let's query if the task is exist in rollup storage with another rollup client
        let mut another_client =
            SubstrateRollupClient::new("http://127.0.0.1:39933", 100, &contract_id).unwrap();
        let onchain_task = OnchainTasks::lookup_task(&mut another_client, &task.id).unwrap();
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
