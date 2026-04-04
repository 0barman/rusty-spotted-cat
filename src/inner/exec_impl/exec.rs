use crate::direction::Direction;
use crate::error::Error;
use crate::inner::active_state::ActiveState;
use crate::inner::chunk_outcome::ChunkOutcome;
use crate::inner::inner_task::InnerTask;
use crate::inner::prepare_outcome::PrepareOutcome;
use crate::inner::transfer_scheduler_state::TransferSchedulerState;
use crate::inner::worker_event::WorkerEvent;
use crate::inner::UniqueId;
use crate::transfer_executor_trait::TransferTrait;
use crate::transfer_status::TransferStatus;
use crate::transfer_task::TransferTask;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) async fn try_start_next(
    worker_tx: &mpsc::Sender<WorkerEvent>,
    state: &mut TransferSchedulerState,
    executor: &Arc<dyn TransferTrait>,
) -> HashSet<UniqueId> {
    let mut started_keys = HashSet::new();
    loop {
        let queued_len = state.queued().len();
        if queued_len == 0 {
            break;
        }

        let mut scheduled_in_this_round = false;
        for _ in 0..queued_len {
            let Some(key) = state.queued_mut().pop_front() else {
                break;
            };
            state.queued_set_mut().remove(&key);

            if state.active().contains_key(&key) {
                continue;
            }
            let Some(group) = state.groups().get(&key) else {
                continue;
            };
            let direction = key.0;
            if !can_start_direction(state, direction) {
                state.queued_mut().push_back(key.clone());
                state.queued_set_mut().insert(key);
                continue;
            }

            let inner = group.leader_inner().clone();
            let current = state.offsets().get(&key).copied().unwrap_or(0);
            crate::inner::exec_impl::emit::emit_status(
                &group.entry(),
                TransferStatus::Transmission,
                current,
                group.entry().inner().total_size(),
            );

            let cancel = CancellationToken::new();
            state
                .active_mut()
                .insert(key.clone(), ActiveState::new(cancel.clone()));
            started_keys.insert(key.clone());
            scheduled_in_this_round = true;

            let worker_tx_clone = worker_tx.clone();
            let executor = executor.clone();
            let start_offset = state.offsets().get(&key).copied().unwrap_or(0);
            tokio::spawn(async move {
                run_group(key, inner, cancel, worker_tx_clone, executor, start_offset).await;
            });
        }

        if !scheduled_in_this_round {
            break;
        }
    }

    started_keys
}

fn can_start_direction(state: &TransferSchedulerState, direction: Direction) -> bool {
    let active = state
        .active()
        .keys()
        .filter(|(d, _)| *d == direction)
        .count();
    match direction {
        Direction::Upload => active < state.max_upload_concurrency(),
        Direction::Download => active < state.max_download_concurrency(),
    }
}

async fn run_group(
    key: UniqueId,
    inner: InnerTask,
    cancel: CancellationToken,
    worker_tx: mpsc::Sender<WorkerEvent>,
    executor: Arc<dyn TransferTrait>,
    start_offset: u64,
) {
    let task = TransferTask::from_inner(&inner);
    let offset_ret = executor.prepare(&task, start_offset).await;
    let PrepareOutcome {
        next_offset,
        total_size: prep_total,
    } = match offset_ret {
        Ok(v) => v,
        Err(e) => {
            let _ = worker_tx.send(WorkerEvent::Failed { key, error: e }).await;
            return;
        }
    };
    let mut offset = next_offset;
    let mut known_total = if prep_total > 0 {
        prep_total
    } else {
        inner.total_size()
    };
    loop {
        if cancel.is_cancelled() {
            let _ = worker_tx.send(WorkerEvent::Canceled { key }).await;
            return;
        }
        let chunk_ret: Result<ChunkOutcome, Error> = executor
            .transfer_chunk(&task, offset, inner.chunk_size(), known_total)
            .await;
        let outcome = match chunk_ret {
            Ok(v) => v,
            Err(e) => {
                let _ = worker_tx.send(WorkerEvent::Failed { key, error: e }).await;
                return;
            }
        };
        if outcome.total_size > 0 {
            known_total = outcome.total_size;
        }
        offset = outcome.next_offset;
        let _ = worker_tx
            .send(WorkerEvent::Progress {
                key: key.clone(),
                next_offset: outcome.next_offset,
                total_size: known_total,
            })
            .await;
        if outcome.done {
            let _ = worker_tx
                .send(WorkerEvent::Completed {
                    key,
                    total_size: known_total,
                })
                .await;
            return;
        }
    }
}
