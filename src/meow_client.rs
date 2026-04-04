use std::sync::{Arc, OnceLock};

use uuid::Uuid;

use crate::dflt::default_http_client::{default_breakpoint_arcs, DefaultHttpClient};
use crate::engine_config::EngineConfig;
use crate::error::{ECode, Error};
use crate::file_transfer_record::FileTransferRecord;
use crate::inner::executor::{Executor, ProgressCb};
use crate::inner::inner_task::InnerTask;
use crate::inner::task_callbacks::TaskCallbacks;
use crate::inner::transfer_snapshot::TransferSnapshot;
use crate::pounce_task::PounceTask;

/// 细化错误码
/// 日志
/// 存储
/// uuid包装
/// EngineConfig
/// 上传 下载参数区分开
/// 上传。下载 的default中性能问题
/// 上传 下载的default作为可配置的模块
/// 关掉用不到的 feature（
#[derive(Debug, Clone)]
pub struct MeowClient {
    executor: OnceLock<Executor>,
    config: EngineConfig,
}

impl MeowClient {
    pub fn new(config: EngineConfig) -> Self {
        MeowClient {
            executor: Default::default(),
            config,
        }
    }

    fn get_exec(&self) -> &Executor {
        self.executor.get_or_init(|| {
            let default = DefaultHttpClient::new();
            Executor::new(self.config.clone(), Arc::new(default))
        })
    }
}

impl MeowClient {
    pub async fn enqueue<PCB>(&self, task: PounceTask, progress_cb: PCB) -> Result<Uuid, Error>
    where
        PCB: Fn(FileTransferRecord) + Send + Sync + 'static,
    {
        if task.is_empty() {
            return Err(Error::from_code1(ECode::ParameterEmpty));
        }

        let progress: ProgressCb = Arc::new(progress_cb);
        let callbacks = TaskCallbacks::new(Some(progress));

        let (def_up, def_down) = default_breakpoint_arcs();
        let inner = InnerTask::from_pounce(
            task,
            self.config.breakpoint_download_http().clone(),
            self.config.http_client_ref().cloned(),
            def_up,
            def_down,
        )
        .await?;

        let id = inner.uuid();
        self.get_exec().enqueue(inner, callbacks)?;
        Ok(id)
    }

    pub(crate) async fn pause(&self, uuid: Uuid) -> Result<(), Error> {
        self.get_exec().pause(uuid).await
    }

    pub(crate) async fn cancel(&self, uuid: Uuid) -> Result<(), Error> {
        self.get_exec().cancel(uuid).await
    }

    pub(crate) async fn snapshot(&self) -> Result<TransferSnapshot, Error> {
        self.get_exec().snapshot().await
    }

    pub(crate) async fn shutdown(&self) -> Result<(), Error> {
        self.get_exec().shutdown().await
    }
}
