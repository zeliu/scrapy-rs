use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use url::Url;

use crate::error::{Error, Result};

/// HTTP methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Method {
    #[default]
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
    PATCH,
}

/// Represents an HTTP request to be made by the crawler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// The URL to request
    pub url: Url,

    /// The HTTP method to use
    #[serde(default)]
    pub method: Method,

    /// HTTP headers to include
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Request body (for POST, PUT, etc.)
    #[serde(default)]
    pub body: Option<Vec<u8>>,

    /// Metadata associated with this request
    #[serde(default)]
    pub meta: HashMap<String, serde_json::Value>,

    /// Priority of this request (higher values = higher priority)
    #[serde(default)]
    pub priority: i32,

    /// Whether to follow redirects
    #[serde(default = "default_follow_redirects")]
    pub follow_redirects: bool,

    /// Maximum number of redirects to follow
    #[serde(default = "default_max_redirects")]
    pub max_redirects: u32,

    /// Callback name to be called when the response is received
    #[serde(default)]
    pub callback: Option<String>,

    /// Error callback name to be called when the request fails
    #[serde(default)]
    pub errback: Option<String>,

    /// Cookies to be sent with the request
    #[serde(default)]
    pub cookies: HashMap<String, String>,

    /// Whether to filter this request (if false, duplicates are allowed)
    #[serde(default = "default_dont_filter")]
    pub dont_filter: bool,

    /// Timeout for this request
    #[serde(default)]
    pub timeout: Option<Duration>,

    /// Encoding for the request
    #[serde(default)]
    pub encoding: Option<String>,

    /// Custom flags for middleware processing
    #[serde(default)]
    pub flags: Vec<String>,

    /// Proxy URL to use for this request
    #[serde(default)]
    pub proxy: Option<String>,

    /// Download latency (filled after download)
    #[serde(default, skip_serializing)]
    pub download_latency: Option<Duration>,
}

fn default_follow_redirects() -> bool {
    true
}

fn default_max_redirects() -> u32 {
    10
}

fn default_dont_filter() -> bool {
    false
}

impl Request {
    /// Create a new GET request
    pub fn get<U: AsRef<str>>(url: U) -> Result<Self> {
        let url = Url::parse(url.as_ref()).map_err(Error::UrlParseError)?;
        Ok(Self {
            url,
            method: Method::GET,
            headers: HashMap::new(),
            body: None,
            meta: HashMap::new(),
            priority: 0,
            follow_redirects: default_follow_redirects(),
            max_redirects: default_max_redirects(),
            callback: None,
            errback: None,
            cookies: HashMap::new(),
            dont_filter: default_dont_filter(),
            timeout: None,
            encoding: None,
            flags: Vec::new(),
            proxy: None,
            download_latency: None,
        })
    }

    /// Create a new POST request
    pub fn post<U: AsRef<str>, B: Into<Vec<u8>>>(url: U, body: B) -> Result<Self> {
        let url = Url::parse(url.as_ref()).map_err(Error::UrlParseError)?;
        Ok(Self {
            url,
            method: Method::POST,
            headers: HashMap::new(),
            body: Some(body.into()),
            meta: HashMap::new(),
            priority: 0,
            follow_redirects: default_follow_redirects(),
            max_redirects: default_max_redirects(),
            callback: None,
            errback: None,
            cookies: HashMap::new(),
            dont_filter: default_dont_filter(),
            timeout: None,
            encoding: None,
            flags: Vec::new(),
            proxy: None,
            download_latency: None,
        })
    }

    /// Add a header to the request
    pub fn with_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Add metadata to the request
    pub fn with_meta<K: Into<String>, V: Into<serde_json::Value>>(
        mut self,
        key: K,
        value: V,
    ) -> Self {
        self.meta.insert(key.into(), value.into());
        self
    }

    /// Set the callback for this request
    pub fn with_callback<C: Into<String>>(mut self, callback: C) -> Self {
        self.callback = Some(callback.into());
        self
    }

    /// Set the errback for this request
    pub fn with_errback<C: Into<String>>(mut self, errback: C) -> Self {
        self.errback = Some(errback.into());
        self
    }

    /// Set the priority for this request
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Add a cookie to the request
    pub fn with_cookie<K: Into<String>, V: Into<String>>(mut self, name: K, value: V) -> Self {
        self.cookies.insert(name.into(), value.into());
        self
    }

    /// Set whether to filter this request
    pub fn with_dont_filter(mut self, dont_filter: bool) -> Self {
        self.dont_filter = dont_filter;
        self
    }

    /// Set the timeout for this request
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the encoding for this request
    pub fn with_encoding<E: Into<String>>(mut self, encoding: E) -> Self {
        self.encoding = Some(encoding.into());
        self
    }

    /// Add a flag to the request
    pub fn with_flag<F: Into<String>>(mut self, flag: F) -> Self {
        self.flags.push(flag.into());
        self
    }

    /// Set the proxy for this request
    pub fn with_proxy<P: Into<String>>(mut self, proxy: P) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    /// Record download latency
    pub fn record_latency(&mut self, latency: Duration) {
        self.download_latency = Some(latency);
    }

    /// Get the effective timeout for this request
    pub fn get_timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Check if a flag is present
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
}

impl PartialEq for Request {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url && self.method == other.method
    }
}

impl Eq for Request {}

impl Hash for Request {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.url.as_str().hash(state);
        std::mem::discriminant(&self.method).hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_request_get() {
        let req = Request::get("https://example.com").unwrap();
        assert_eq!(req.url.as_str(), "https://example.com/");
        assert_eq!(req.method, Method::GET);
        assert!(req.body.is_none());
    }

    #[test]
    fn test_request_post() {
        let body = "test body";
        let req = Request::post("https://example.com", body).unwrap();
        assert_eq!(req.url.as_str(), "https://example.com/");
        assert_eq!(req.method, Method::POST);
        assert_eq!(req.body.unwrap(), body.as_bytes());
    }

    #[test]
    fn test_request_with_header() {
        let req = Request::get("https://example.com")
            .unwrap()
            .with_header("User-Agent", "scrapy-rs/0.1.0");

        assert_eq!(req.headers.get("User-Agent").unwrap(), "scrapy-rs/0.1.0");
    }

    #[test]
    fn test_request_with_meta() {
        let req = Request::get("https://example.com")
            .unwrap()
            .with_meta("depth", 2);

        assert_eq!(req.meta.get("depth").unwrap(), &serde_json::json!(2));
    }

    #[test]
    fn test_request_with_cookies() {
        let req = Request::get("https://example.com")
            .unwrap()
            .with_cookie("session", "abc123");

        assert_eq!(req.cookies.get("session").unwrap(), "abc123");
    }

    #[test]
    fn test_request_with_dont_filter() {
        let req = Request::get("https://example.com")
            .unwrap()
            .with_dont_filter(true);

        assert!(req.dont_filter);
    }

    #[test]
    fn test_request_with_timeout() {
        let timeout = Duration::from_secs(30);
        let req = Request::get("https://example.com")
            .unwrap()
            .with_timeout(timeout);

        assert_eq!(req.timeout, Some(timeout));
    }

    #[test]
    fn test_request_with_flags() {
        let req = Request::get("https://example.com")
            .unwrap()
            .with_flag("high-priority")
            .with_flag("no-cache");

        assert!(req.has_flag("high-priority"));
        assert!(req.has_flag("no-cache"));
        assert!(!req.has_flag("non-existent"));
    }

    #[test]
    fn test_request_with_proxy() {
        let req = Request::get("https://example.com")
            .unwrap()
            .with_proxy("http://proxy.example.com:8080");

        assert_eq!(req.proxy.unwrap(), "http://proxy.example.com:8080");
    }

    #[test]
    fn test_record_latency() {
        let mut req = Request::get("https://example.com").unwrap();
        let latency = Duration::from_millis(150);

        req.record_latency(latency);
        assert_eq!(req.download_latency, Some(latency));
    }
}
