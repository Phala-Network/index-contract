use alloc::vec::Vec;

fn add_task(&self, task: &Task) -> Result<()> {
    let local_tasks = pink_extension::ext()
        .cache_get(b"running_tasks")
        .ok_or(Error::ReadCacheFailed)?;
    let mut decoded_tasks: Vec<TaskId> = Decode::decode(&mut local_tasks.as_slice())
        .map_err(|_| Error::DecodeCacheFailed)?;

    if !decoded_tasks.contains(&task.id) {
        decoded_tasks.push(task.id);
        pink_extension::ext()
            .cache_set(b"running_tasks", &decoded_tasks.encode())
            .map_err(|_| Error::WriteCacheFailed)?;
        // Save full task information
        pink_extension::ext()
            .cache_set(&task.id, &task.encode())
            .map_err(|_| Error::WriteCacheFailed)?;
    }
    Ok(())
}

fn remove_task(&self, task: &Task) -> Result<()> {
    let local_tasks = pink_extension::ext()
        .cache_get(b"running_tasks")
        .ok_or(Error::ReadCacheFailed)?;
    let mut decoded_tasks: Vec<TaskId> = Decode::decode(&mut local_tasks.as_slice())
        .map_err(|_| Error::DecodeCacheFailed)?;
    let index = decoded_tasks
        .iter()
        .position(|id| *id == task.id)
        .ok_or(Error::TaskNotFoundInCache)?;
    decoded_tasks.remove(index);
    // Delete task record from cache
    pink_extension::ext()
        .cache_remove(&task.id)
        .ok_or(Error::WriteCacheFailed)?;

    Ok(())
}

fn update_task(&self, task: &Task) -> Result<()> {
    if let Some(_) = pink_extension::ext().cache_get(&task.id) {
        // Update task record
        pink_extension::ext()
            .cache_set(&task.id, &task.encode())
            .map_err(|_| Error::WriteCacheFailed)?;
    }
    Ok(())
}

fn get_task(&self, id: &TaskId) -> Option<Task> {
    pink_extension::ext()
        .cache_get(id)
        .and_then(|encoded_task| {
            match Decode::decode(&mut encoded_task.as_slice())
                .map_err(|_| Error::DecodeCacheFailed)
            {
                Ok(task) => Some(task),
                _ => None,
            }
        })
}
