use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Method;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::direction::Direction;
use crate::error::{ECode, Error};
use crate::http_breakpoint::{
    BreakpointDownload, BreakpointUpload, DefaultStyleUpload, StandardRangeDownload,
};
use crate::inner::chunk_outcome::ChunkOutcome;
use crate::inner::prepare_outcome::PrepareOutcome;
use crate::transfer_executor_trait::TransferTrait;
use crate::transfer_task::TransferTask;

pub(crate) fn default_breakpoint_arcs() -> (
    Arc<dyn BreakpointUpload + Send + Sync>,
    Arc<dyn BreakpointDownload + Send + Sync>,
) {
    (
        Arc::new(DefaultStyleUpload::default()),
        Arc::new(StandardRangeDownload::default()),
    )
}

/// 基于 reqwest 的断点上传（multipart）与断点下载（HEAD + Range GET），具体表单与 Header 由
/// [`TransferTask`] 上的头、`BreakpointUpload` / `BreakpointDownload` 与 `DefaultHttpClient` 的 fallback 共同决定。
pub struct DefaultHttpClient {
    client: reqwest::Client,
    fallback_upload: Arc<dyn BreakpointUpload + Send + Sync>,
    fallback_download: Arc<dyn BreakpointDownload + Send + Sync>,
}

impl DefaultHttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            fallback_upload: Arc::new(DefaultStyleUpload::default()),
            fallback_download: Arc::new(StandardRangeDownload::default()),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client,
            fallback_upload: Arc::new(DefaultStyleUpload::default()),
            fallback_download: Arc::new(StandardRangeDownload::default()),
        }
    }

    /// 自定义全局默认的断点协议（任务级 [`TransferTask::with_breakpoint_upload`] 仍可覆盖）。
    pub fn with_fallbacks(
        client: reqwest::Client,
        upload: Arc<dyn BreakpointUpload + Send + Sync>,
        download: Arc<dyn BreakpointDownload + Send + Sync>,
    ) -> Self {
        Self {
            client,
            fallback_upload: upload,
            fallback_download: download,
        }
    }

    fn client_for(&self, task: &TransferTask) -> reqwest::Client {
        task.http_client_ref()
            .cloned()
            .unwrap_or_else(|| self.client.clone())
    }

    fn upload_arc(&self, task: &TransferTask) -> Arc<dyn BreakpointUpload + Send + Sync> {
        match task.breakpoint_upload() {
            Some(a) => a.clone(),
            None => self.fallback_upload.clone(),
        }
    }

    fn download_arc(&self, task: &TransferTask) -> Arc<dyn BreakpointDownload + Send + Sync> {
        match task.breakpoint_download() {
            Some(a) => a.clone(),
            None => self.fallback_download.clone(),
        }
    }
}

impl Default for DefaultHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

async fn upload_prepare(
    client: &reqwest::Client,
    task: &TransferTask,
    upload: Arc<dyn BreakpointUpload + Send + Sync>,
    local_offset: u64,
) -> Result<PrepareOutcome, Error> {
    let form = upload.prepare_multipart(task);
    let method = task.method();
    let headers = task.headers().clone();
    let req = client
        .request(method.clone(), task.url())
        .headers(headers)
        .multipart(form);
    let resp = req.send().await.map_err(map_reqwest)?;
    let status = resp.status();
    let body = resp.text().await.map_err(map_reqwest)?;
    if !status.is_success() {
        return Err(Error::from_code(
            ECode::HttpError,
            format!("upload prepare HTTP {status}: {body}"),
        ));
    }
    let info = upload.parse_upload_response(&body)?;
    if info.completed_file_id.is_some() {
        let total = task.total_size();
        return Ok(PrepareOutcome {
            next_offset: total,
            total_size: total,
        });
    }
    let server_off = info.next_byte.unwrap_or(0);
    let next = local_offset.max(server_off).min(task.total_size());
    Ok(PrepareOutcome {
        next_offset: next,
        total_size: task.total_size(),
    })
}

async fn download_prepare(
    client: &reqwest::Client,
    task: &TransferTask,
    download: Arc<dyn BreakpointDownload + Send + Sync>,
    _local_offset: u64,
) -> Result<PrepareOutcome, Error> {
    let path = task.file_path();
    let local_len = if path.exists() {
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;
        meta.len()
    } else {
        0u64
    };

    // 与`check_resume` 一致：以本地已落盘长度作为续传起点，避免与调度器 offset 不一致产生空洞。
    let start = local_len;
    let head_url = download.head_url(task);
    let mut head_headers = task.headers().clone();
    download.merge_head_headers(task, &mut head_headers);
    let head_resp = client
        .request(Method::HEAD, &head_url)
        .headers(head_headers)
        .send()
        .await
        .map_err(map_reqwest)?;
    if !head_resp.status().is_success() {
        return Err(Error::from_code(
            ECode::HttpError,
            format!("HEAD failed: {}", head_resp.status()),
        ));
    }
    let total = download.total_size_from_head(head_resp.headers())?;
    if start > total {
        return Err(Error::from_code_str(
            ECode::IoError,
            "local file larger than remote content-length",
        ));
    }
    if start >= total {
        return Ok(PrepareOutcome {
            next_offset: total,
            total_size: total,
        });
    }
    Ok(PrepareOutcome {
        next_offset: start,
        total_size: total,
    })
}

async fn upload_one_chunk(
    client: &reqwest::Client,
    task: &TransferTask,
    upload: Arc<dyn BreakpointUpload + Send + Sync>,
    offset: u64,
    chunk_size: u64,
) -> Result<ChunkOutcome, Error> {
    let total = task.total_size();
    if offset >= total {
        return Ok(ChunkOutcome {
            next_offset: offset,
            total_size: total,
            done: true,
        });
    }
    let read_len = chunk_size.min(total - offset);
    if read_len == 0 {
        return Ok(ChunkOutcome {
            next_offset: offset,
            total_size: total,
            done: true,
        });
    }

    let path = task.file_path();
    let mut file = File::open(path)
        .await
        .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;
    file.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;
    let mut buf = vec![0u8; read_len as usize];
    file.read_exact(&mut buf)
        .await
        .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;

    let form = upload.chunk_multipart(task, &buf, offset)?;
    let headers = task.headers().clone();
    let resp = client
        .request(task.method(), task.url())
        .headers(headers)
        .multipart(form)
        .send()
        .await
        .map_err(map_reqwest)?;
    let status = resp.status();
    let body = resp.text().await.map_err(map_reqwest)?;
    if !status.is_success() {
        return Err(Error::from_code(
            ECode::HttpError,
            format!("upload chunk HTTP {status}: {body}"),
        ));
    }
    let info = upload.parse_upload_response(&body)?;
    if info.completed_file_id.is_some() {
        return Ok(ChunkOutcome {
            next_offset: total,
            total_size: total,
            done: true,
        });
    }
    let next = offset + buf.len() as u64;
    Ok(ChunkOutcome {
        next_offset: next,
        total_size: total,
        done: next >= total,
    })
}

fn range_spec(start: u64, chunk_size: u64, total: u64) -> String {
    if total == 0 {
        return format!("bytes={start}-");
    }
    let end = (start + chunk_size - 1).min(total.saturating_sub(1));
    format!("bytes={start}-{end}")
}

async fn download_one_chunk(
    client: &reqwest::Client,
    task: &TransferTask,
    download: Arc<dyn BreakpointDownload + Send + Sync>,
    offset: u64,
    chunk_size: u64,
    remote_total_size: u64,
) -> Result<ChunkOutcome, Error> {
    let total = remote_total_size;
    if offset >= total {
        return Ok(ChunkOutcome {
            next_offset: offset,
            total_size: total,
            done: true,
        });
    }

    let spec = range_spec(offset, chunk_size, total);
    let mut headers = task.headers().clone();
    download.merge_range_get_headers(task, &spec, &mut headers);

    let resp = client
        .get(task.url())
        .headers(headers)
        .send()
        .await
        .map_err(map_reqwest)?;
    let status = resp.status();
    if !(status.is_success() || status == reqwest::StatusCode::PARTIAL_CONTENT) {
        let body = resp.text().await.unwrap_or_default();
        return Err(Error::from_code(
            ECode::HttpError,
            format!("download GET {status}: {body}"),
        ));
    }
    let data = resp.bytes().await.map_err(map_reqwest)?;
    if data.is_empty() {
        return Err(Error::from_code_str(
            ECode::HttpError,
            "download chunk empty body",
        ));
    }

    let path = task.file_path();
    if offset == 0 {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;
            }
        }
        let mut f = File::create(path)
            .await
            .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;
        f.write_all(&data)
            .await
            .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;
    } else {
        let mut f = OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .await
            .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;
        f.write_all(&data)
            .await
            .map_err(|e| Error::from_code(ECode::IoError, e.to_string()))?;
    }

    let next = offset + data.len() as u64;
    Ok(ChunkOutcome {
        next_offset: next,
        total_size: total,
        done: next >= total,
    })
}

fn map_reqwest(e: reqwest::Error) -> Error {
    Error::from_code(ECode::HttpError, e.to_string())
}

#[async_trait]
impl TransferTrait for DefaultHttpClient {
    async fn prepare(
        &self,
        task: &TransferTask,
        local_offset: u64,
    ) -> Result<PrepareOutcome, Error> {
        let client = self.client_for(task);
        match task.direction() {
            Direction::Upload => {
                upload_prepare(&client, task, self.upload_arc(task), local_offset).await
            }
            Direction::Download => {
                download_prepare(&client, task, self.download_arc(task), local_offset).await
            }
        }
    }

    async fn transfer_chunk(
        &self,
        task: &TransferTask,
        offset: u64,
        chunk_size: u64,
        remote_total_size: u64,
    ) -> Result<ChunkOutcome, Error> {
        let client = self.client_for(task);
        match task.direction() {
            Direction::Upload => {
                upload_one_chunk(&client, task, self.upload_arc(task), offset, chunk_size).await
            }
            Direction::Download => {
                download_one_chunk(
                    &client,
                    task,
                    self.download_arc(task),
                    offset,
                    chunk_size,
                    remote_total_size,
                )
                .await
            }
        }
    }
}
