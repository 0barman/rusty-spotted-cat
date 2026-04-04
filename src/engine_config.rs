use crate::http_breakpoint::BreakpointDownloadHttpConfig;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    max_upload_concurrency: usize,
    max_download_concurrency: usize,
    breakpoint_download_http: BreakpointDownloadHttpConfig,
    http_client: Option<reqwest::Client>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_upload_concurrency: 2,
            max_download_concurrency: 2,
            breakpoint_download_http: BreakpointDownloadHttpConfig::default(),
            http_client: None,
        }
    }
}

impl EngineConfig {
    pub fn new(max_upload_concurrency: usize, max_download_concurrency: usize) -> Self {
        Self {
            max_upload_concurrency,
            max_download_concurrency,
            breakpoint_download_http: BreakpointDownloadHttpConfig::default(),
            http_client: None,
        }
    }

    pub fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    pub fn max_upload_concurrency(&self) -> usize {
        self.max_upload_concurrency
    }

    pub fn max_download_concurrency(&self) -> usize {
        self.max_download_concurrency
    }

    pub fn breakpoint_download_http(&self) -> &BreakpointDownloadHttpConfig {
        &self.breakpoint_download_http
    }

    pub fn with_breakpoint_download_http(mut self, config: BreakpointDownloadHttpConfig) -> Self {
        self.breakpoint_download_http = config;
        self
    }

    pub(crate) fn http_client_ref(&self) -> Option<&reqwest::Client> {
        self.http_client.as_ref()
    }
}
