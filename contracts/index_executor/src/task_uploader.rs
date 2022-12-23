use super::types::Task;
use scale::{Decode, Encode};

#[derive(Debug, PartialEq, Eq, Encode, Decode, Clone)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct UploadToChain {
    /// Task to be uploaded
    pub task: Task,
}
