use crate::inner::group_state::RecordEntry;
use crate::transfer_status::TransferStatus;

pub(crate) fn emit_status(
    entry: &RecordEntry,
    status: TransferStatus,
    transferred: u64,
    total: u64,
) {
    let inner = entry.inner();
    let dto = crate::file_transfer_record::FileTransferRecord::new(
        inner.uuid(),
        inner.file_sign().to_string(),
        inner.file_name().to_string(),
        total,
        if total == 0 {
            0.0
        } else {
            transferred as f32 / total as f32
        },
        status,
        inner.direction(),
    );
    if let Some(cb) = &entry.callbacks().progress_cb() {
        cb(dto);
    }
}
