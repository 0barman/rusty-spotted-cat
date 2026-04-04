use std::path::PathBuf;
use std::sync::Arc;

use reqwest::header::HeaderMap;
use reqwest::Method;

use crate::direction::Direction;
use crate::http_breakpoint::{BreakpointDownload, BreakpointDownloadHttpConfig, BreakpointUpload};

/// 仅承载调用方入参，由 [`UploadPounceBuilder`] / [`DownloadPounceBuilder`] 构造；引擎入队时再转为内部 [`crate::inner::inner_task::InnerTask`]。
#[derive(Clone)]
pub struct PounceTask {
    pub(crate) direction: Direction,
    pub(crate) file_name: String,
    pub(crate) file_path: PathBuf,
    pub(crate) total_size: u64,
    pub(crate) chunk_size: u64,
    pub(crate) url: String,
    pub(crate) method: Method,
    pub(crate) headers: HeaderMap,
    /// 下载任务在进度回调里展示的标识；上传任务忽略，由内部 MD5 填充。
    pub(crate) client_file_sign: Option<String>,
    pub(crate) breakpoint_upload: Option<Arc<dyn BreakpointUpload + Send + Sync>>,
    pub(crate) breakpoint_download: Option<Arc<dyn BreakpointDownload + Send + Sync>>,
    pub(crate) breakpoint_download_http: Option<BreakpointDownloadHttpConfig>,
}

impl PounceTask {
    pub(crate) fn normalized_chunk_size(chunk_size: u64) -> u64 {
        if chunk_size == 0 {
            1024 * 1024
        } else {
            chunk_size
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.file_path.as_os_str().is_empty()
            || self.file_name.is_empty()
            || self.url.is_empty()
            || match self.direction {
                Direction::Upload => self.total_size == 0,
                Direction::Download => false,
            }
    }
}
