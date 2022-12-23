use crate::task::{Task, TaskId};
use alloc::vec::Vec;
use scale::{Decode, Encode};

pub fn add_task_local(task: &Task) -> Result<(), &'static str> {
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
    Ok(())
}

pub fn remove_task_local(task: &Task) -> Result<(), &'static str> {
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

    Ok(())
}

pub fn update_task_local(task: &Task) -> Result<(), &'static str> {
    if let Some(_) = pink_extension::ext().cache_get(&task.id) {
        // Update task record
        pink_extension::ext()
            .cache_set(&task.id, &task.encode())
            .map_err(|_| "WriteCacheFailed")?;
    }
    Ok(())
}

pub fn get_task_local(id: &TaskId) -> Option<Task> {
    pink_extension::ext()
        .cache_get(id)
        .and_then(
            |encoded_task| match Decode::decode(&mut encoded_task.as_slice()) {
                Ok(task) => Some(task),
                _ => None,
            },
        )
}

pub fn get_all_task_local() -> Result<Vec<TaskId>, &'static str> {
    let local_tasks = pink_extension::ext()
        .cache_get(b"running_tasks")
        .ok_or("ReadCacheFailed")?;
    let decoded_tasks: Vec<TaskId> =
        Decode::decode(&mut local_tasks.as_slice()).map_err(|_| "DecodeCacheFailed")?;
    Ok(decoded_tasks)
}
