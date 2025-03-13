use std::collections::HashMap;

use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::{Error, Result};
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_downloader::Downloader;

/// A mock downloader for testing
pub struct MockDownloader {
    /// Responses to return for specific URLs
    responses: HashMap<String, Response>,
}

impl MockDownloader {
    /// Create a new mock downloader
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    /// Add a response for a specific URL
    #[allow(dead_code)]
    pub fn add_response(&mut self, url: &str, response: Response) {
        self.responses.insert(url.to_string(), response);
    }
}

#[async_trait]
impl Downloader for MockDownloader {
    async fn download(&self, request: Request) -> Result<Response> {
        let url = request.url.to_string();
        if let Some(response) = self.responses.get(&url) {
            Ok(response.clone())
        } else {
            // Create a default response
            let mut headers = HashMap::new();
            headers.insert("Content-Type".to_string(), "text/html".to_string());
            let body = format!(
                "<html><body><h1>Mock response for {}</h1></body></html>",
                url
            )
            .into_bytes();
            Ok(Response::new(request, 200, headers, body))
        }
    }
}

/// A mock downloader that always fails
pub struct FailingDownloader;

#[async_trait]
impl Downloader for FailingDownloader {
    async fn download(&self, _request: Request) -> Result<Response> {
        Err(Box::new(Error::other("Mock downloader failure")))
    }
}
