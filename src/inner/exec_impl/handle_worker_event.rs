use crate::transfer_status::TransferStatus;

use crate::inner::transfer_scheduler_state::TransferSchedulerState;
use crate::inner::worker_event::WorkerEvent;

pub(crate) async fn handle_worker_event(event: WorkerEvent, state: &mut TransferSchedulerState) {
    match event {
        WorkerEvent::Progress {
            key,
            next_offset,
            total_size,
        } => {
            state.offsets_mut().insert(key.clone(), next_offset);
            if let Some(group) = state.groups().get(&key) {
                crate::inner::exec_impl::emit::emit_status(
                    group.entry(),
                    TransferStatus::Transmission,
                    next_offset,
                    total_size,
                );
            }
        }
        WorkerEvent::Completed { key, total_size } => {
            state.active_mut().remove(&key);
            state.offsets_mut().insert(key.clone(), total_size);
            if let Some(group) = state.groups().get(&key) {
                crate::inner::exec_impl::emit::emit_status(
                    group.entry(),
                    TransferStatus::Complete,
                    total_size,
                    total_size,
                );
            }
        }
        WorkerEvent::Failed { key, error } => {
            state.active_mut().remove(&key);
            if let Some(group) = state.groups().get(&key) {
                let current = state.offsets().get(&key).copied().unwrap_or(0);
                crate::inner::exec_impl::emit::emit_status(
                    group.entry(),
                    TransferStatus::Failed(error),
                    current,
                    group.entry().inner().total_size(),
                );
            }
        }
        WorkerEvent::Canceled { key } => {
            state.active_mut().remove(&key);
            if let Some(group) = state.groups().get(&key) {
                let current = state.offsets().get(&key).copied().unwrap_or(0);
                crate::inner::exec_impl::emit::emit_status(
                    group.entry(),
                    TransferStatus::Canceled,
                    current,
                    group.entry().inner().total_size(),
                );
            }
        }
    }
}
