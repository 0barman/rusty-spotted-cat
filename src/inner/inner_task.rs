use std::path::{Path, PathBuf};
use std::sync::Arc;

use reqwest::header::HeaderMap;
use reqwest::Method;
use tokio::fs::File;
use uuid::Uuid;

use crate::direction::Direction;
use crate::error::{ECode, Error};
use crate::http_breakpoint::{BreakpointDownload, BreakpointDownloadHttpConfig, BreakpointUpload};
use crate::inner::sign::calculate_sign;
use crate::inner::UniqueId;
use crate::pounce_task::PounceTask;

/// 调度与 [`crate::transfer_executor_trait::TransferTrait`] 视图的内部任务；不对外暴露构造。
#[derive(Clone)]
pub(crate) struct InnerTask {
    uuid: Uuid,
    file_sign: String,
    file_name: String,
    file_path: PathBuf,
    direction: Direction,
    total_size: u64,
    chunk_size: u64,
    url: String,
    method: Method,
    headers: HeaderMap,
    breakpoint_upload: Arc<dyn BreakpointUpload + Send + Sync>,
    breakpoint_download: Arc<dyn BreakpointDownload + Send + Sync>,
    breakpoint_download_http: BreakpointDownloadHttpConfig,
    http_client: Option<reqwest::Client>,
}

impl InnerTask {
    pub(crate) async fn from_pounce(
        pounce: PounceTask,
        default_download_http: BreakpointDownloadHttpConfig,
        http_client: Option<reqwest::Client>,
        default_upload: Arc<dyn BreakpointUpload + Send + Sync>,
        default_download: Arc<dyn BreakpointDownload + Send + Sync>,
    ) -> Result<Self, Error> {
        let uuid = Uuid::now_v7();

        let PounceTask {
            direction,
            file_name,
            file_path,
            total_size,
            chunk_size,
            url,
            method,
            headers,
            client_file_sign,
            breakpoint_upload,
            breakpoint_download,
            breakpoint_download_http,
        } = pounce;

        let file_sign = match direction {
            Direction::Upload => {
                let file = File::open(&file_path)
                    .await
                    .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;
                calculate_sign(&file).await?
            }
            Direction::Download => client_file_sign.unwrap_or_default(),
        };

        let breakpoint_upload = breakpoint_upload.unwrap_or(default_upload);
        let breakpoint_download = breakpoint_download.unwrap_or(default_download);
        let breakpoint_download_http = breakpoint_download_http.unwrap_or(default_download_http);

        Ok(Self {
            uuid,
            file_sign,
            file_name,
            file_path,
            direction,
            total_size,
            chunk_size,
            url,
            method,
            headers,
            breakpoint_upload,
            breakpoint_download,
            breakpoint_download_http,
            http_client,
        })
    }

    pub(crate) fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub(crate) fn dedupe_key(&self) -> UniqueId {
        match self.direction {
            Direction::Upload => (Direction::Upload, self.file_sign.clone()),
            Direction::Download => (Direction::Download, self.url.clone()),
        }
    }

    pub(crate) fn file_sign(&self) -> &str {
        &self.file_sign
    }

    pub(crate) fn file_name(&self) -> &str {
        &self.file_name
    }

    pub(crate) fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub(crate) fn direction(&self) -> Direction {
        self.direction
    }

    pub(crate) fn total_size(&self) -> u64 {
        self.total_size
    }

    pub(crate) fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn method(&self) -> Method {
        self.method.clone()
    }

    pub(crate) fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub(crate) fn breakpoint_download_http(&self) -> &BreakpointDownloadHttpConfig {
        &self.breakpoint_download_http
    }

    pub(crate) fn breakpoint_upload(&self) -> &Arc<dyn BreakpointUpload + Send + Sync> {
        &self.breakpoint_upload
    }

    pub(crate) fn breakpoint_download(&self) -> &Arc<dyn BreakpointDownload + Send + Sync> {
        &self.breakpoint_download
    }

    pub(crate) fn http_client_ref(&self) -> Option<&reqwest::Client> {
        self.http_client.as_ref()
    }
}
