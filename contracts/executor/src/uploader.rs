use super::types::Task;
use scale::{Decode, Encode};

#[derive(Debug, PartialEq, Eq, Encode, Decode, Clone)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct UploadToChain {
    /// Task to be uploaded
    pub task: Task,
}

/// Update/Create task in rollup storage.
///
/// Return transaction hash if success.
struct TaskUploader;
impl TaskUploader {
    pub fn upload_task(worker: &AccountId, task: &Task) -> Result<Vec<u8>, Error> {
        Err(Error::Unimplemented)
    }
}
