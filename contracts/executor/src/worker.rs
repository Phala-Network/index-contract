

pub fn init_worker_alloc() {
    let empty_alloc: Vec<[u8; 32]> = vec![];
    pink_extension::ext()
        .cache_set(b"alloc", &empty_alloc.encode())
        .expect("write cache failed");
}

/// Mark an account as allocated, e.g. put it into local cache `alloc` queue.
#[allow(dead_code)]
fn allocate_worker(&self, worker: &[u8; 32]) -> Result<()> {
    let alloc_list = pink_extension::ext()
        .cache_get(b"alloc")
        .ok_or(Error::ReadCacheFailed)?;
    let mut decoded_list: Vec<[u8; 32]> =
        Decode::decode(&mut alloc_list.as_slice()).map_err(|_| Error::DecodeCacheFailed)?;

    decoded_list.push(*worker);
    pink_extension::ext()
        .cache_set(b"alloc", &decoded_list.encode())
        .map_err(|_| Error::WriteCacheFailed)?;
    Ok(())
}

/// Retuen accounts that hasn't been allocated to a specific task
#[allow(dead_code)]
fn free_worker(&self) -> Result<Vec<[u8; 32]>> {
    let mut free_list = vec![];
    let alloc_list = pink_extension::ext()
        .cache_get(b"alloc")
        .ok_or(Error::ReadCacheFailed)?;
    let decoded_list: Vec<[u8; 32]> =
        Decode::decode(&mut alloc_list.as_slice()).map_err(|_| Error::DecodeCacheFailed)?;

    for worker in self.worker_accounts.iter() {
        if !decoded_list.contains(worker) {
            free_list.push(*worker);
        }
    }
    Ok(free_list)
}
