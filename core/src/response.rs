use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

use crate::error::{Error, ResponseParseError, Result};
use crate::request::Request;

/// Represents an HTTP response received by the crawler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// The URL of the response
    pub url: Url,

    /// The HTTP status code
    pub status: u16,

    /// HTTP headers received
    pub headers: HashMap<String, String>,

    /// Response body
    pub body: Vec<u8>,

    /// The request that generated this response
    pub request: Request,

    /// Metadata associated with this response
    #[serde(default)]
    pub meta: HashMap<String, serde_json::Value>,
}

impl Response {
    /// Create a new response
    pub fn new(
        request: Request,
        status: u16,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            url: request.url.clone(),
            status,
            headers,
            body,
            request,
            meta: HashMap::new(),
        }
    }

    /// Get the response body as a string
    pub fn text(&self) -> Result<String> {
        Ok(String::from_utf8(self.body.clone()).map_err(|e| {
            Error::parse(ResponseParseError::Other(format!(
                "Failed to decode UTF-8: {}",
                e
            )))
        })?)
    }

    /// Parse the response body as JSON
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        let text = self.text()?;
        serde_json::from_str(&text).map_err(|e| Box::new(Error::SerdeError(e.to_string())))
    }

    /// Add metadata to the response
    pub fn with_meta<K: Into<String>, V: Into<serde_json::Value>>(
        mut self,
        key: K,
        value: V,
    ) -> Self {
        self.meta.insert(key.into(), value.into());
        self
    }

    /// Check if the response was successful (status code 200-299)
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Check if the response is a redirect
    pub fn is_redirect(&self) -> bool {
        matches!(self.status, 301 | 302 | 303 | 307 | 308)
    }

    /// Get the redirect URL if this response is a redirect
    pub fn redirect_url(&self) -> Option<Result<Url>> {
        if !self.is_redirect() {
            return None;
        }

        self.headers.get("location").map(|location| {
            let base_url = &self.url;
            base_url
                .join(location)
                .map_err(|e| Box::new(Error::UrlParseError(e)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_text() {
        let request = Request::get("https://example.com").unwrap();
        let body = "Hello, world!".as_bytes().to_vec();
        let response = Response::new(request, 200, HashMap::new(), body.clone());

        assert_eq!(response.text().unwrap(), "Hello, world!");
    }

    #[test]
    fn test_response_json() {
        let request = Request::get("https://example.com").unwrap();
        let body = r#"{"message": "Hello, world!"}"#.as_bytes().to_vec();
        let response = Response::new(request, 200, HashMap::new(), body);

        let json: serde_json::Value = response.json().unwrap();
        assert_eq!(json["message"], "Hello, world!");
    }

    #[test]
    fn test_response_is_success() {
        let request = Request::get("https://example.com").unwrap();
        let response = Response::new(request.clone(), 200, HashMap::new(), Vec::new());
        assert!(response.is_success());

        let response = Response::new(request, 404, HashMap::new(), Vec::new());
        assert!(!response.is_success());
    }

    #[test]
    fn test_response_redirect_url() {
        let request = Request::get("https://example.com").unwrap();
        let mut headers = HashMap::new();
        headers.insert("location".to_string(), "/new-page".to_string());

        let response = Response::new(request, 301, headers, Vec::new());

        assert!(response.is_redirect());
        let redirect_url = response.redirect_url().unwrap().unwrap();
        assert_eq!(redirect_url.as_str(), "https://example.com/new-page");
    }
}
