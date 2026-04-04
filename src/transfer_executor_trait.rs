use async_trait::async_trait;

use crate::error::Error;
use crate::inner::chunk_outcome::ChunkOutcome;
use crate::inner::prepare_outcome::PrepareOutcome;
use crate::transfer_task::TransferTask;

#[async_trait]
pub trait TransferTrait: Send + Sync {
    async fn prepare(
        &self,
        task: &TransferTask,
        local_offset: u64,
    ) -> Result<PrepareOutcome, Error>;

    /// `remote_total_size`：下载时为 [`PrepareOutcome::total_size`]（HEAD 得到）；上传可与 `task.total_size()` 一致。
    async fn transfer_chunk(
        &self,
        task: &TransferTask,
        offset: u64,
        chunk_size: u64,
        remote_total_size: u64,
    ) -> Result<ChunkOutcome, Error>;
}
