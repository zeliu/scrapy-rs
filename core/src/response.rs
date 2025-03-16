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

    /// Flags to indicate various processing states
    #[serde(default)]
    pub flags: Vec<String>,

    /// SSL certificate information (DER encoded)
    #[serde(default)]
    pub certificate: Option<Vec<u8>>,

    /// IP address of the server that returned the response
    #[serde(default)]
    pub ip_address: Option<String>,

    /// Protocol used for the request (HTTP, HTTPS)
    #[serde(default)]
    pub protocol: Option<String>,
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
            flags: Vec::new(),
            certificate: None,
            ip_address: None,
            protocol: None,
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

    /// Add a flag to the response
    pub fn with_flag<F: Into<String>>(mut self, flag: F) -> Self {
        self.flags.push(flag.into());
        self
    }

    /// Set the certificate for the response
    pub fn with_certificate(mut self, certificate: Vec<u8>) -> Self {
        self.certificate = Some(certificate);
        self
    }

    /// Set the IP address for the response
    pub fn with_ip_address<I: Into<String>>(mut self, ip_address: I) -> Self {
        self.ip_address = Some(ip_address.into());
        self
    }

    /// Set the protocol for the response
    pub fn with_protocol<P: Into<String>>(mut self, protocol: P) -> Self {
        self.protocol = Some(protocol.into());
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

    /// Create a copy of this response
    pub fn copy(&self) -> Self {
        self.clone()
    }

    /// Create a new response with modified attributes
    #[allow(clippy::too_many_arguments)]
    pub fn replace(
        &self,
        url: Option<Url>,
        status: Option<u16>,
        headers: Option<HashMap<String, String>>,
        body: Option<Vec<u8>>,
        request: Option<Request>,
        meta: Option<HashMap<String, serde_json::Value>>,
        flags: Option<Vec<String>>,
        certificate: Option<Option<Vec<u8>>>,
        ip_address: Option<Option<String>>,
        protocol: Option<Option<String>>,
    ) -> Self {
        Self {
            url: url.unwrap_or_else(|| self.url.clone()),
            status: status.unwrap_or(self.status),
            headers: headers.unwrap_or_else(|| self.headers.clone()),
            body: body.unwrap_or_else(|| self.body.clone()),
            request: request.unwrap_or_else(|| self.request.clone()),
            meta: meta.unwrap_or_else(|| self.meta.clone()),
            flags: flags.unwrap_or_else(|| self.flags.clone()),
            certificate: certificate.unwrap_or(self.certificate.clone()),
            ip_address: ip_address.unwrap_or(self.ip_address.clone()),
            protocol: protocol.unwrap_or(self.protocol.clone()),
        }
    }

    /// Join this response's URL with a relative URL
    pub fn urljoin<U: AsRef<str>>(&self, url: U) -> Result<Url> {
        self.url
            .join(url.as_ref())
            .map_err(|e| Box::new(Error::UrlParseError(e)))
    }

    /// Create a new request from a URL found in this response
    pub fn follow<U: AsRef<str>>(&self, url: U) -> Result<Request> {
        let absolute_url = self.urljoin(url)?;
        let mut request = Request::get(absolute_url.as_str())?;

        // Copy relevant information from the original request
        if let Some(ref_depth) = self.request.meta.get("depth") {
            if let Some(depth) = ref_depth.as_i64() {
                request = request.with_meta("depth", depth + 1);
            }
        }

        // Copy cookies from the original request
        for (name, value) in &self.request.cookies {
            request.cookies.insert(name.clone(), value.clone());
        }

        Ok(request)
    }

    /// Create multiple requests from URLs found in this response
    pub fn follow_all<U, I>(&self, urls: I) -> Result<Vec<Request>>
    where
        U: AsRef<str>,
        I: IntoIterator<Item = U>,
    {
        let mut requests = Vec::new();
        for url in urls {
            requests.push(self.follow(url)?);
        }
        Ok(requests)
    }

    /// Check if a flag is present
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
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

    #[test]
    fn test_response_with_flags() {
        let request = Request::get("https://example.com").unwrap();
        let response = Response::new(request, 200, HashMap::new(), Vec::new())
            .with_flag("cached")
            .with_flag("from-middleware");

        assert!(response.has_flag("cached"));
        assert!(response.has_flag("from-middleware"));
        assert!(!response.has_flag("non-existent"));
        assert_eq!(response.flags.len(), 2);
    }

    #[test]
    fn test_response_copy_and_replace() {
        let request = Request::get("https://example.com").unwrap();
        let original = Response::new(request.clone(), 200, HashMap::new(), b"original".to_vec())
            .with_flag("original");

        // Test copy
        let copied = original.copy();
        assert_eq!(copied.url, original.url);
        assert_eq!(copied.status, original.status);
        assert_eq!(copied.body, original.body);
        assert_eq!(copied.flags, original.flags);

        // Test replace
        let replaced = original.replace(
            None,
            Some(404),
            None,
            Some(b"replaced".to_vec()),
            None,
            None,
            Some(vec!["replaced".to_string()]),
            None,
            None,
            None,
        );

        assert_eq!(replaced.url, original.url); // Same URL
        assert_eq!(replaced.status, 404); // Changed status
        assert_eq!(replaced.body, b"replaced"); // Changed body
        assert_eq!(replaced.flags, vec!["replaced"]); // Changed flags
    }

    #[test]
    fn test_response_urljoin() {
        let request = Request::get("https://example.com/page").unwrap();
        let response = Response::new(request, 200, HashMap::new(), Vec::new());

        let absolute = response.urljoin("/absolute").unwrap();
        assert_eq!(absolute.as_str(), "https://example.com/absolute");

        let relative = response.urljoin("relative").unwrap();
        assert_eq!(relative.as_str(), "https://example.com/relative");

        let with_query = response.urljoin("?query=value").unwrap();
        assert_eq!(with_query.as_str(), "https://example.com/page?query=value");
    }

    #[test]
    fn test_response_follow() {
        let mut request = Request::get("https://example.com").unwrap();
        request
            .meta
            .insert("depth".to_string(), serde_json::json!(1));
        request
            .cookies
            .insert("session".to_string(), "abc123".to_string());

        let response = Response::new(request, 200, HashMap::new(), Vec::new());

        let followed = response.follow("/next-page").unwrap();

        assert_eq!(followed.url.as_str(), "https://example.com/next-page");
        assert_eq!(followed.meta.get("depth").unwrap(), &serde_json::json!(2)); // Depth incremented
        assert_eq!(followed.cookies.get("session").unwrap(), "abc123"); // Cookies copied
    }

    #[test]
    fn test_response_follow_all() {
        let request = Request::get("https://example.com").unwrap();
        let response = Response::new(request, 200, HashMap::new(), Vec::new());

        let urls = vec!["/page1", "/page2", "/page3"];
        let followed = response.follow_all(urls).unwrap();

        assert_eq!(followed.len(), 3);
        assert_eq!(followed[0].url.as_str(), "https://example.com/page1");
        assert_eq!(followed[1].url.as_str(), "https://example.com/page2");
        assert_eq!(followed[2].url.as_str(), "https://example.com/page3");
    }
}
