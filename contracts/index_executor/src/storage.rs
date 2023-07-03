use crate::task::{Task, TaskId};
use alloc::{string::String, vec, vec::Vec};

pub struct StorageClient {
    url: String,
    key: String,
}

impl StorageClient {
    pub fn new(url: String, key: String) -> Self {
        StorageClient { url, key }
    }

    pub fn put(&self, key: &[u8], data: &Vec<u8>) -> Result<(), &'static str> {
        Err("Unimplemented")
    }

    pub fn get(&self, key: &[u8]) -> Result<Vec<u8>, &'static str> {
        Err("Unimplemented")
    }

    pub fn delete(&self, key: &[u8]) -> Result<(), &'static str> {
        Err("Unimplemented")
    }

    pub fn upload_task(&self, task: &Task) -> Result<(), &'static str> {
        Err("Unimplemented")
    }

    pub fn lookup_task(&self, id: &TaskId) -> Option<Task> {
        // TODO
        None
    }

    pub fn lookup_pending_tasks(&self) -> Vec<TaskId> {
        // TODO
        vec![]
    }

    pub fn lookup_free_accounts(&self) -> Option<Vec<[u8; 32]>> {
        // TODO
        None
    }

    pub fn set_worker_accounts(&self, accounts: Vec<[u8; 32]>) {
        // TODO
    }
}
