use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use url::Url;

use crate::error::{Error, Result};

/// HTTP methods supported by the crawler
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
    PATCH,
}

impl Default for Method {
    fn default() -> Self {
        Method::GET
    }
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
}

fn default_follow_redirects() -> bool {
    true
}

fn default_max_redirects() -> u32 {
    10
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
        })
    }

    /// Add a header to the request
    pub fn with_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Add metadata to the request
    pub fn with_meta<K: Into<String>, V: Into<serde_json::Value>>(mut self, key: K, value: V) -> Self {
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
            .with_header("User-Agent", "rs-spider/0.1.0");
        
        assert_eq!(req.headers.get("User-Agent").unwrap(), "rs-spider/0.1.0");
    }

    #[test]
    fn test_request_with_meta() {
        let req = Request::get("https://example.com")
            .unwrap()
            .with_meta("depth", 2);
        
        assert_eq!(req.meta.get("depth").unwrap(), &serde_json::json!(2));
    }
} 