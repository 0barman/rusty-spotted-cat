use crate::error::Error;

use super::UniqueId;

#[derive(Debug)]
pub(crate) enum WorkerEvent {
    Progress {
        key: UniqueId,
        next_offset: u64,
        total_size: u64,
    },
    Completed {
        key: UniqueId,
        total_size: u64,
    },
    Failed {
        key: UniqueId,
        error: Error,
    },
    Canceled {
        key: UniqueId,
    },
}
