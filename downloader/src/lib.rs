use std::sync::Arc;
use std::time::Duration;

use backoff::{ExponentialBackoff, ExponentialBackoffBuilder};
use futures::future::join_all;
use log::debug;
use reqwest::Client;
use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::{Error, HttpError, Result};
use scrapy_rs_core::request::{Method, Request};
use scrapy_rs_core::response::Response;
use tokio::sync::Semaphore;
use tokio::time::sleep;

/// Configuration for the downloader
#[derive(Debug, Clone)]
pub struct DownloaderConfig {
    /// Maximum number of concurrent requests
    pub concurrent_requests: usize,

    /// User agent string
    pub user_agent: String,

    /// Default request timeout in seconds
    pub timeout: u64,

    /// Whether to retry failed requests
    pub retry_enabled: bool,

    /// Maximum number of retries
    pub max_retries: u32,

    /// Initial retry delay in milliseconds
    pub retry_delay_ms: u64,

    /// Maximum retry delay in milliseconds
    pub max_retry_delay_ms: u64,

    /// Retry backoff factor
    pub retry_backoff_factor: f64,
}

impl Default for DownloaderConfig {
    fn default() -> Self {
        Self {
            concurrent_requests: 10,
            user_agent: format!("scrapy_rs/{}", env!("CARGO_PKG_VERSION")),
            timeout: 30,
            retry_enabled: true,
            max_retries: 3,
            retry_delay_ms: 1000,
            max_retry_delay_ms: 30000,
            retry_backoff_factor: 2.0,
        }
    }
}

/// Trait for downloaders
#[async_trait]
pub trait Downloader: Send + Sync + 'static {
    /// Download a single request
    async fn download(&self, request: Request) -> Result<Response>;

    /// Download multiple requests concurrently
    async fn download_many(&self, requests: Vec<Request>) -> Vec<Result<Response>> {
        let futures = requests.into_iter().map(|req| self.download(req));
        join_all(futures).await
    }
}

/// HTTP downloader implementation
pub struct HttpDownloader {
    /// HTTP client
    client: Client,

    /// Semaphore to limit concurrent requests
    semaphore: Arc<Semaphore>,

    /// Downloader configuration
    config: DownloaderConfig,
}

impl HttpDownloader {
    /// Create a new HTTP downloader with the given configuration
    pub fn new(config: DownloaderConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(config.timeout))
            .build()
            .map_err(|e| Error::other(format!("Failed to create HTTP client: {}", e)))?;

        let semaphore = Arc::new(Semaphore::new(config.concurrent_requests));

        Ok(Self {
            client,
            semaphore,
            config,
        })
    }

    /// Create a retry backoff strategy
    #[allow(dead_code)]
    fn create_backoff(&self) -> ExponentialBackoff {
        ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(self.config.retry_delay_ms))
            .with_max_interval(Duration::from_millis(self.config.max_retry_delay_ms))
            .with_multiplier(self.config.retry_backoff_factor)
            .with_max_elapsed_time(Some(Duration::from_secs(self.config.timeout * 2)))
            .build()
    }

    #[allow(dead_code)]
    async fn download_with_retries(&self, request: Request) -> Result<Response> {
        let mut retries = 0;
        let max_retries = if self.config.retry_enabled {
            self.config.max_retries
        } else {
            0
        };

        loop {
            match self.download(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if retries >= max_retries || !e.is_retryable() {
                        return Err(e);
                    }

                    retries += 1;
                    debug!(
                        "Retrying request to {} ({}/{})",
                        request.url, retries, max_retries
                    );

                    // Apply exponential backoff
                    let delay =
                        Duration::from_millis(self.config.retry_delay_ms * 2u64.pow(retries - 1));
                    sleep(delay).await;
                }
            }
        }
    }

    fn build_reqwest_request(&self, request: &Request) -> Result<reqwest::RequestBuilder> {
        // Create a reqwest request builder
        let req_builder = self.client.request(
            match request.method {
                Method::GET => reqwest::Method::GET,
                Method::POST => reqwest::Method::POST,
                Method::PUT => reqwest::Method::PUT,
                Method::DELETE => reqwest::Method::DELETE,
                Method::HEAD => reqwest::Method::HEAD,
                Method::OPTIONS => reqwest::Method::OPTIONS,
                Method::PATCH => reqwest::Method::PATCH,
            },
            request.url.clone(),
        );

        // Add headers
        let mut req_builder = req_builder;
        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        Ok(req_builder)
    }
}

impl Default for HttpDownloader {
    /// Create a new HTTP downloader with default configuration
    fn default() -> Self {
        Self::new(DownloaderConfig::default()).expect("Failed to create default HttpDownloader")
    }
}

#[async_trait]
impl Downloader for HttpDownloader {
    async fn download(&self, request: Request) -> Result<Response> {
        // Acquire a permit from the semaphore
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| Error::other(format!("Failed to acquire semaphore permit: {}", e)))?;

        debug!("Downloading URL: {}", request.url);

        // Build the reqwest request
        let req_builder = self.build_reqwest_request(&request)?;

        // Send the request
        let response = if let Some(body) = &request.body {
            req_builder.body(body.clone()).send().await.map_err(|e| {
                Error::http(HttpError::ClientError {
                    status: 0,
                    message: format!("HTTP request failed: {}", e),
                })
            })
        } else {
            req_builder.send().await.map_err(|e| {
                Error::http(HttpError::ClientError {
                    status: 0,
                    message: format!("HTTP request failed: {}", e),
                })
            })
        }?;

        // Get the status code
        let status = response.status().as_u16();

        // Get the headers
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_string()))
            .collect();

        // Get the body
        let body = response
            .bytes()
            .await
            .map_err(|e| {
                Error::http(HttpError::ClientError {
                    status: 0,
                    message: format!("Failed to read response body: {}", e),
                })
            })?
            .to_vec();

        // Create the response
        let rs_response = Response::new(request, status, headers, body);

        Ok(rs_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_http_downloader() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Mock a successful response
        Mock::given(method("GET"))
            .and(path("/success"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Success"))
            .mount(&mock_server)
            .await;

        // Mock a 404 response
        Mock::given(method("GET"))
            .and(path("/not-found"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .mount(&mock_server)
            .await;

        // Create a downloader with a small concurrency limit
        let config = DownloaderConfig {
            concurrent_requests: 2,
            ..DownloaderConfig::default()
        };
        let downloader = HttpDownloader::new(config).unwrap();

        // Test a successful request
        let request = Request::get(format!("{}/success", mock_server.uri())).unwrap();
        let response = downloader.download(request).await.unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.text().unwrap(), "Success");

        // Test a 404 request
        let request = Request::get(format!("{}/not-found", mock_server.uri())).unwrap();
        let response = downloader.download(request).await.unwrap();

        assert_eq!(response.status, 404);
        assert_eq!(response.text().unwrap(), "Not Found");

        // Test downloading multiple requests
        let requests = vec![
            Request::get(format!("{}/success", mock_server.uri())).unwrap(),
            Request::get(format!("{}/not-found", mock_server.uri())).unwrap(),
        ];

        let responses = downloader.download_many(requests).await;
        assert_eq!(responses.len(), 2);

        let success_response = &responses[0].as_ref().unwrap();
        assert_eq!(success_response.status, 200);

        let not_found_response = &responses[1].as_ref().unwrap();
        assert_eq!(not_found_response.status, 404);
    }
}

#[cfg(test)]
pub mod mock;
