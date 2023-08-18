use crate::account::AccountInfo;
use crate::chain::{Chain, ChainType};
use crate::task::Task;
use crate::task_deposit::{DepositData, EvmDepositData, SubDepositData};
use alloc::vec::Vec;
use pink_extension::ResultExt;
use pink_subrpc::{
    get_storage,
    hasher::Twox64Concat,
    storage::{storage_map_prefix, storage_prefix},
};
use pink_web3::{
    api::{Eth, Namespace},
    contract::{Contract, Options},
    transports::{resolve_ready, PinkHttp},
    types::{Address, H160},
};

/// Fetch actived tasks from blockchains and construct a `Task` from it.
/// If the given chain is EVM based, fetch tasks from solidity-based smart contract storage through RPC task.
/// For example, call RPC methods defined here:
///     https://github.com/Phala-Network/index-solidity/blob/7b4458f9b8277df8a1c705a4d0f264476f42fcf2/contracts/Handler.sol#L147
///     https://github.com/Phala-Network/index-solidity/blob/7b4458f9b8277df8a1c705a4d0f264476f42fcf2/contracts/Handler.sol#L165
/// If the given chain is Substrate based, fetch tasks from pallet storage through RPC task.
pub struct ActivedTaskFetcher {
    pub chain: Chain,
    pub worker: AccountInfo,
}
impl ActivedTaskFetcher {
    pub fn new(chain: Chain, worker: AccountInfo) -> Self {
        ActivedTaskFetcher { chain, worker }
    }

    pub fn fetch_task(&self) -> Result<Option<Task>, &'static str> {
        match self.chain.chain_type {
            ChainType::Evm => Ok(self.query_evm_actived_task(&self.chain, &self.worker)?),
            ChainType::Sub => Ok(self.query_sub_actived_task(&self.chain, &self.worker)?),
        }
    }

    fn query_evm_actived_task(
        &self,
        chain: &Chain,
        worker: &AccountInfo,
    ) -> Result<Option<Task>, &'static str> {
        let handler_on_goerli: H160 = H160::from_slice(&chain.handler_contract);
        let transport = Eth::new(PinkHttp::new(&chain.endpoint));
        let handler = Contract::from_json(
            transport,
            handler_on_goerli,
            include_bytes!("./abi/handler.json"),
        )
        .map_err(|_| "ConstructContractFailed")?;

        let worker_address: Address = worker.account20.into();
        pink_extension::debug!(
            "Lookup actived task for worker {:?} on {:?}",
            &hex::encode(worker_address),
            &chain.name
        );

        let task_id: [u8; 32] = resolve_ready(handler.query(
            "getLastActivedTask",
            worker_address,
            None,
            Options::default(),
            None,
        ))
        .map_err(|_| "FailedGetLastActivedTask")?;
        if task_id == [0; 32] {
            return Ok(None);
        }
        pink_extension::debug!(
            "getLastActivedTask, return task_id: {:?}",
            hex::encode(task_id)
        );
        let evm_deposit_data: EvmDepositData =
            resolve_ready(handler.query("getTaskData", task_id, None, Options::default(), None))
                .map_err(|_| "FailedGetTaskData")?;
        pink_extension::debug!(
            "Fetch deposit data successfully for task {:?} on {:?}, deposit data: {:?}",
            &hex::encode(task_id),
            &chain.name,
            &evm_deposit_data,
        );
        let deposit_data: DepositData = evm_deposit_data.into();
        let task = deposit_data.to_task(&chain.name, task_id, self.worker.account32)?;
        Ok(Some(task))
    }

    fn query_sub_actived_task(
        &self,
        chain: &Chain,
        worker: &AccountInfo,
    ) -> Result<Option<Task>, &'static str> {
        if let Some(raw_storage) = get_storage(
            &chain.endpoint,
            &storage_map_prefix::<Twox64Concat>(
                &storage_prefix("PalletIndex", "ActivedTasks")[..],
                &worker.account32,
            ),
            None,
        )
        .log_err("Read storage [actived task] failed")
        .map_err(|_| "FailedGetTaskData")?
        {
            let actived_tasks: Vec<[u8; 32]> = scale::Decode::decode(&mut raw_storage.as_slice())
                .log_err("Decode storage [actived task] failed")
                .map_err(|_| "DecodeStorageFailed")?;
            if !actived_tasks.is_empty() {
                let oldest_task = actived_tasks[0];
                if let Some(raw_storage) = get_storage(
                    &chain.endpoint,
                    &storage_map_prefix::<Twox64Concat>(
                        &storage_prefix("PalletIndex", "DepositRecords")[..],
                        &oldest_task,
                    ),
                    None,
                )
                .log_err("Read storage [actived task] failed")
                .map_err(|_| "FailedGetDepositData")?
                {
                    let sub_deposit_data: SubDepositData =
                        scale::Decode::decode(&mut raw_storage.as_slice())
                            .log_err("Decode storage [deposit data] failed")
                            .map_err(|_| "DecodeStorageFailed")?;
                    pink_extension::debug!(
                        "Fetch deposit data successfully for task {:?} on {:?}, deposit data: {:?}",
                        &hex::encode(oldest_task),
                        &chain.name,
                        &sub_deposit_data,
                    );
                    let deposit_data: DepositData = sub_deposit_data.into();
                    let task =
                        deposit_data.to_task(&chain.name, oldest_task, self.worker.account32)?;
                    Ok(Some(task))
                } else {
                    Err("DepositInfoNotFound")
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}
