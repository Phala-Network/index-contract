use crate::task::{Task, TaskId};
use alloc::vec::Vec;
use scale::{Decode, Encode};

pub struct TaskCache;

impl TaskCache {
    pub fn add_task(task: &Task) -> Result<(), &'static str> {
        pink_extension::debug!("add_task: begin");
        let local_tasks = pink_extension::ext()
            .cache_get(b"running_tasks")
            .ok_or("ReadCacheFailed")?;
        let mut decoded_tasks: Vec<TaskId> =
            Decode::decode(&mut local_tasks.as_slice()).map_err(|_| "DecodeCacheFailed")?;

        if !decoded_tasks.contains(&task.id) {
            decoded_tasks.push(task.id);
            pink_extension::ext()
                .cache_set(b"running_tasks", &decoded_tasks.encode())
                .map_err(|_| "WriteCacheFailed")?;
            // Save full task information
            pink_extension::ext()
                .cache_set(&task.id, &task.encode())
                .map_err(|_| "WriteCacheFailed")?;
        }
        pink_extension::debug!("add_task: end");
        Ok(())
    }

    pub fn remove_task(task: &Task) -> Result<(), &'static str> {
        pink_extension::debug!("remove_task: begin");
        let local_tasks = pink_extension::ext()
            .cache_get(b"running_tasks")
            .ok_or("ReadCacheFailed")?;
        let mut decoded_tasks: Vec<TaskId> =
            Decode::decode(&mut local_tasks.as_slice()).map_err(|_| "DecodeCacheFailed")?;
        let index = decoded_tasks
            .iter()
            .position(|id| *id == task.id)
            .ok_or("TaskNotFoundInCache")?;
        decoded_tasks.remove(index);
        // Delete task record from cache
        pink_extension::ext()
            .cache_remove(&task.id)
            .ok_or("WriteCacheFailed")?;
        // Update runing task list
        pink_extension::ext()
            .cache_set(b"running_tasks", &decoded_tasks.encode())
            .map_err(|_| "WriteCacheFailed")?;
        pink_extension::debug!("remove_task: end");
        Ok(())
    }

    pub fn update_task(task: &Task) -> Result<(), &'static str> {
        pink_extension::debug!("update_task: begin");
        if pink_extension::ext().cache_get(&task.id).is_some() {
            // Update task record
            pink_extension::ext()
                .cache_set(&task.id, &task.encode())
                .map_err(|_| "WriteCacheFailed")?;
        }
        pink_extension::debug!("update_task: end");
        Ok(())
    }

    pub fn get_task(id: &TaskId) -> Option<Task> {
        pink_extension::debug!("get_task: begin");
        pink_extension::ext()
            .cache_get(id)
            .and_then(
                |encoded_task| match Decode::decode(&mut encoded_task.as_slice()) {
                    Ok(task) => Some(task),
                    _ => None,
                },
            )
    }
}
