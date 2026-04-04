use crate::direction::Direction;
use crate::http_breakpoint::{BreakpointDownload, BreakpointDownloadHttpConfig};
use crate::pounce_task::PounceTask;
use reqwest::header::HeaderMap;
use reqwest::Method;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct DownloadPounceBuilder {
    file_name: String,
    file_path: PathBuf,
    chunk_size: u64,
    url: String,
    method: Method,
    headers: HeaderMap,
    client_file_sign: Option<String>,
    breakpoint_download: Option<Arc<dyn BreakpointDownload + Send + Sync>>,
    breakpoint_download_http: Option<BreakpointDownloadHttpConfig>,
}

impl DownloadPounceBuilder {
    /// 是否重复任务仅由 `url` 判定（与方向组合）。
    pub fn new(
        file_name: impl Into<String>,
        file_path: impl AsRef<Path>,
        chunk_size: u64,
        url: impl Into<String>,
        method: Method,
    ) -> Self {
        Self {
            file_name: file_name.into(),
            file_path: file_path.as_ref().to_path_buf(),
            chunk_size: PounceTask::normalized_chunk_size(chunk_size),
            url: url.into(),
            method,
            headers: HeaderMap::new(),
            client_file_sign: None,
            breakpoint_download: None,
            breakpoint_download_http: None,
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    pub fn with_file_path(mut self, path: impl AsRef<Path>) -> Self {
        self.file_path = path.as_ref().to_path_buf();
        self
    }

    pub fn with_method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_client_file_sign(mut self, sign: impl Into<String>) -> Self {
        self.client_file_sign = Some(sign.into());
        self
    }

    pub fn with_breakpoint_download(
        mut self,
        download: Arc<dyn BreakpointDownload + Send + Sync>,
    ) -> Self {
        self.breakpoint_download = Some(download);
        self
    }

    pub fn with_breakpoint_download_http(mut self, config: BreakpointDownloadHttpConfig) -> Self {
        self.breakpoint_download_http = Some(config);
        self
    }

    pub fn build(self) -> PounceTask {
        PounceTask {
            direction: Direction::Download,
            file_name: self.file_name,
            file_path: self.file_path,
            total_size: 0,
            chunk_size: self.chunk_size,
            url: self.url,
            method: self.method,
            headers: self.headers,
            client_file_sign: self.client_file_sign,
            breakpoint_upload: None,
            breakpoint_download: self.breakpoint_download,
            breakpoint_download_http: self.breakpoint_download_http,
        }
    }
}
