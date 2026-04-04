use crate::engine_config::EngineConfig;
use crate::error::{ECode, Error};
use crate::file_transfer_record::FileTransferRecord;
use crate::inner::group_state::{GroupState, RecordEntry};
use crate::inner::inner_task::InnerTask;
use crate::inner::task_callbacks::TaskCallbacks;
use crate::inner::transfer_scheduler_state::TransferSchedulerState;
use crate::inner::transfer_snapshot::TransferSnapshot;
use crate::inner::worker_event::WorkerEvent;
use crate::inner::UniqueId;
use crate::transfer_executor_trait::TransferTrait;
use crate::transfer_status::TransferStatus;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub(crate) type ProgressCb = Arc<dyn Fn(FileTransferRecord) + Send + Sync + 'static>;

pub(crate) enum TransferCmd {
    Enqueue {
        inner: InnerTask,
        callbacks: TaskCallbacks,
    },
    Pause {
        uuid: Uuid,
    },
    Cancel {
        uuid: Uuid,
    },
    Snapshot {
        respond_to: tokio::sync::oneshot::Sender<TransferSnapshot>,
    },
    Shutdown,
}

fn worker_loop(
    mut cmd_rx: mpsc::Receiver<TransferCmd>,
    mut worker_rx: mpsc::Receiver<WorkerEvent>,
    worker_tx: mpsc::Sender<WorkerEvent>,
    mut state: TransferSchedulerState,
    executor: Arc<dyn TransferTrait>,
) {
    std::thread::spawn(move || {
        let runtime_ret = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build();
        if let Ok(runtime) = runtime_ret {
            runtime.block_on(async move {
                loop {
                    tokio::select! {
                        biased;
                        maybe_cmd = cmd_rx.recv() => {
                            let Some(cmd) = maybe_cmd else { break; };
                            match cmd {
                                TransferCmd::Enqueue { inner, callbacks } => {
                                    let key = inner.dedupe_key();
                                    if state.groups().contains_key(&key) {
                                        if let Some(cb) = &callbacks.progress_cb() {
                                            let leader = state.groups().get(&key).unwrap().leader_inner();
                                            cb(to_record_inner(
                                                leader,
                                                TransferStatus::Failed(Error::from_code1(
                                                    ECode::DuplicateTaskError,
                                                )),
                                                0,
                                                leader.total_size(),
                                            ));
                                        }
                                        continue;
                                    }

                                    state
                                        .uuid_to_dedupe_mut()
                                        .insert(inner.uuid(), key.clone());

                                    let should_send_running = state.active().contains_key(&key);
                                    let entry = RecordEntry::new(inner.clone(), callbacks);
                                    state.groups_mut().insert(
                                        key.clone(),
                                        GroupState::new(inner.clone(), entry),
                                    );

                                    if let Some(group) = state.groups().get(&key) {
                                        let current = state.offsets().get(&key).copied().unwrap_or(0);
                                        crate::inner::exec_impl::emit::emit_status(
                                            group.entry(),
                                            TransferStatus::Pending,
                                            current,
                                            group.entry().inner().total_size(),
                                        );
                                        if should_send_running {
                                            crate::inner::exec_impl::emit::emit_status(
                                                group.entry(),
                                                TransferStatus::Transmission,
                                                current,
                                                group.entry().inner().total_size(),
                                            );
                                        }
                                    }

                                    if !state.active().contains_key(&key) && !state.queued_set().contains(&key) {
                                        state.queued_mut().push_back(key.clone());
                                        state.queued_set_mut().insert(key.clone());
                                    }
                                    let _ = crate::inner::exec_impl::exec::try_start_next(
                                        &worker_tx,
                                        &mut state,
                                        &executor,
                                    )
                                    .await;
                                }
                                TransferCmd::Pause { uuid } => {
                                    if let Some(key) = state.uuid_to_dedupe().get(&uuid).cloned() {
                                        pause_group(&mut state, &key).await;
                                    }
                                    let _ = crate::inner::exec_impl::exec::try_start_next(
                                        &worker_tx,
                                        &mut state,
                                        &executor,
                                    )
                                    .await;
                                }
                                TransferCmd::Cancel { uuid } => {
                                    if let Some(key) = state.uuid_to_dedupe().get(&uuid).cloned() {
                                        cancel_group(&mut state, &key).await;
                                    }
                                    let _ = crate::inner::exec_impl::exec::try_start_next(
                                        &worker_tx,
                                        &mut state,
                                        &executor,
                                    )
                                    .await;
                                }
                                TransferCmd::Snapshot { respond_to } => {
                                    let _ = respond_to.send(TransferSnapshot {
                                        queued_groups: state.queued().len(),
                                        active_groups: state.active().len(),
                                        active_keys: state.active().keys().cloned().collect(),
                                    });
                                }
                                TransferCmd::Shutdown => {
                                    for (_, active) in state.active().iter() {
                                        active.cancel().cancel();
                                    }
                                    state.active_mut().clear();
                                    state.groups_mut().clear();
                                    state.uuid_to_dedupe_mut().clear();
                                    state.queued_mut().clear();
                                    state.queued_set_mut().clear();
                                    state.offsets_mut().clear();
                                    break;
                                }
                            }
                        }
                        maybe_event = worker_rx.recv() => {
                            let Some(event) = maybe_event else { continue; };
                            crate::inner::exec_impl::handle_worker_event::handle_worker_event(
                                event,
                                &mut state,
                            )
                            .await;
                            let _ = crate::inner::exec_impl::exec::try_start_next(
                                &worker_tx,
                                &mut state,
                                &executor,
                            )
                            .await;
                        }
                    }
                }
            });
        };
    });
}

async fn pause_group(state: &mut TransferSchedulerState, key: &UniqueId) {
    if let Some(active) = state.active_mut().remove(key) {
        active.cancel().cancel();
    }
    state.queued_mut().retain(|k| k != key);
    state.queued_set_mut().remove(key);

    if let Some(group) = state.groups_mut().remove(key) {
        state
            .uuid_to_dedupe_mut()
            .remove(&group.leader_inner().uuid());
        let entry = group.entry();
        let current = state.offsets().get(key).copied().unwrap_or(0);
        crate::inner::exec_impl::emit::emit_status(
            entry,
            TransferStatus::Paused,
            current,
            entry.inner().total_size(),
        );
    }
}

async fn cancel_group(state: &mut TransferSchedulerState, key: &UniqueId) {
    if let Some(active) = state.active_mut().remove(key) {
        active.cancel().cancel();
    }
    state.queued_mut().retain(|k| k != key);
    state.queued_set_mut().remove(key);
    if let Some(group) = state.groups_mut().remove(key) {
        state
            .uuid_to_dedupe_mut()
            .remove(&group.leader_inner().uuid());
        let entry = group.entry();
        let current = state.offsets().get(key).copied().unwrap_or(0);
        crate::inner::exec_impl::emit::emit_status(
            entry,
            TransferStatus::Canceled,
            current,
            entry.inner().total_size(),
        );
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Executor {
    cmd_tx: tokio::sync::mpsc::Sender<TransferCmd>,
}

impl Executor {
    pub(crate) fn new(config: EngineConfig, executor: Arc<dyn TransferTrait>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<TransferCmd>(256);
        let (worker_tx, worker_rx) = mpsc::channel::<WorkerEvent>(1024);
        worker_loop(
            cmd_rx,
            worker_rx,
            worker_tx,
            TransferSchedulerState::new(
                config.max_upload_concurrency(),
                config.max_download_concurrency(),
            ),
            executor,
        );
        Self { cmd_tx }
    }
}

impl Executor {
    pub(crate) fn enqueue(
        &self,
        inner: InnerTask,
        callbacks: TaskCallbacks,
    ) -> Result<Uuid, Error> {
        let id = inner.uuid();
        self.cmd_tx
            .try_send(TransferCmd::Enqueue { inner, callbacks })
            .map_err(|e| Error::from_code(ECode::EnqueueError, format!("enqueue_failed: {}", e)))?;
        Ok(id)
    }

    pub(crate) async fn pause(&self, uuid: Uuid) -> Result<(), Error> {
        self.cmd_tx
            .send(TransferCmd::Pause { uuid })
            .await
            .map_err(|e| Error::from_code(ECode::EnqueueError, format!("pause_failed: {}", e)))
    }

    pub(crate) async fn cancel(&self, uuid: Uuid) -> Result<(), Error> {
        self.cmd_tx
            .send(TransferCmd::Cancel { uuid })
            .await
            .map_err(|e| Error::from_code(ECode::EnqueueError, format!("cancel_failed: {}", e)))
    }

    pub(crate) async fn snapshot(&self) -> Result<TransferSnapshot, Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(TransferCmd::Snapshot { respond_to: tx })
            .await
            .map_err(|e| {
                Error::from_code(
                    ECode::EnqueueError,
                    format!("snapshot cmd_tx send failed: {}", e),
                )
            })?;
        rx.await.map_err(|e| {
            Error::from_code(
                ECode::EnqueueError,
                format!("snapshot rx.await failed: {}", e),
            )
        })
    }

    pub(crate) async fn shutdown(&self) -> Result<(), Error> {
        self.cmd_tx
            .send(TransferCmd::Shutdown)
            .await
            .map_err(|e| Error::from_code(ECode::EnqueueError, format!("shutdown_failed: {}", e)))
    }
}

fn to_record_inner(
    inner: &InnerTask,
    status: TransferStatus,
    transferred: u64,
    file_size_u64: u64,
) -> FileTransferRecord {
    let progress = if file_size_u64 == 0 {
        0.0
    } else {
        transferred as f32 / file_size_u64 as f32
    };
    FileTransferRecord::new(
        inner.uuid(),
        inner.file_sign().to_string(),
        inner.file_name().to_string(),
        file_size_u64,
        progress,
        status,
        inner.direction(),
    )
}
