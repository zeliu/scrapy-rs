use std::collections::HashMap;
use std::fmt;
use std::time::Duration;
use thiserror::Error;
use url::ParseError;
use serde::{Deserialize, Serialize};

/// Error context information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorContext {
    /// URL associated with the error
    pub url: Option<String>,
    
    /// HTTP status code (if applicable)
    pub status_code: Option<u16>,
    
    /// Request ID (if available)
    pub request_id: Option<String>,
    
    /// Spider name
    pub spider_name: Option<String>,
    
    /// Component where the error occurred
    pub component: Option<String>,
    
    /// Time when the error occurred
    #[serde(skip)]
    pub timestamp: Option<std::time::SystemTime>,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new() -> Self {
        let mut context = Self::default();
        context.timestamp = Some(std::time::SystemTime::now());
        context
    }
    
    /// Set the URL
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }
    
    /// Set the status code
    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = Some(status_code);
        self
    }
    
    /// Set the request ID
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
    
    /// Set the spider name
    pub fn with_spider_name(mut self, spider_name: impl Into<String>) -> Self {
        self.spider_name = Some(spider_name.into());
        self
    }
    
    /// Set the component
    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.component = Some(component.into());
        self
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        
        if let Some(ref url) = self.url {
            parts.push(format!("url={}", url));
        }
        
        if let Some(status_code) = self.status_code {
            parts.push(format!("status={}", status_code));
        }
        
        if let Some(ref request_id) = self.request_id {
            parts.push(format!("request_id={}", request_id));
        }
        
        if let Some(ref spider_name) = self.spider_name {
            parts.push(format!("spider={}", spider_name));
        }
        
        if let Some(ref component) = self.component {
            parts.push(format!("component={}", component));
        }
        
        for (key, value) in &self.metadata {
            parts.push(format!("{}={}", key, value));
        }
        
        write!(f, "{}", parts.join(", "))
    }
}

/// Network error types
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum NetworkError {
    /// Connection refused
    #[error("Connection refused")]
    ConnectionRefused,
    
    /// Connection reset
    #[error("Connection reset")]
    ConnectionReset,
    
    /// Connection timed out
    #[error("Connection timed out")]
    ConnectionTimeout,
    
    /// DNS resolution error
    #[error("DNS resolution error: {0}")]
    DnsError(String),
    
    /// SSL/TLS error
    #[error("SSL/TLS error: {0}")]
    SslError(String),
    
    /// Proxy error
    #[error("Proxy error: {0}")]
    ProxyError(String),
    
    /// Too many redirects
    #[error("Too many redirects")]
    TooManyRedirects,
    
    /// Request timeout
    #[error("Request timeout after {0:?}")]
    Timeout(Duration),
    
    /// Other network error
    #[error("Network error: {0}")]
    Other(String),
}

/// HTTP error types
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum HttpError {
    /// Client error (4xx)
    #[error("Client error: {status} {message}")]
    ClientError {
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
    },
    
    /// Server error (5xx)
    #[error("Server error: {status} {message}")]
    ServerError {
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
    },
    
    /// Rate limited
    #[error("Rate limited: {status} {message}, retry after {retry_after:?}")]
    RateLimited {
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
        /// Retry after duration
        retry_after: Option<Duration>,
    },
    
    /// Redirect error
    #[error("Redirect error: {status} {message}")]
    RedirectError {
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
    },
}

/// Parse error types
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum ResponseParseError {
    /// HTML parse error
    #[error("HTML parse error: {0}")]
    HtmlError(String),
    
    /// JSON parse error
    #[error("JSON parse error: {0}")]
    JsonError(String),
    
    /// XML parse error
    #[error("XML parse error: {0}")]
    XmlError(String),
    
    /// CSS selector error
    #[error("CSS selector error: {0}")]
    CssSelectorError(String),
    
    /// XPath error
    #[error("XPath error: {0}")]
    XPathError(String),
    
    /// Regex error
    #[error("Regex error: {0}")]
    RegexError(String),
    
    /// Other parse error
    #[error("Parse error: {0}")]
    Other(String),
}

/// Error types for the RS-Spider framework
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// Error when parsing a URL
    #[error("URL parse error: {0}")]
    UrlParseError(#[from] ParseError),

    /// Network error
    #[error("Network error: {error} {context}")]
    Network {
        /// The network error
        error: NetworkError,
        /// Error context
        context: ErrorContext,
    },

    /// HTTP error
    #[error("HTTP error: {error} {context}")]
    Http {
        /// The HTTP error
        error: HttpError,
        /// Error context
        context: ErrorContext,
    },

    /// Error when parsing response content
    #[error("Parse error: {error} {context}")]
    Parse {
        /// The parse error
        error: ResponseParseError,
        /// Error context
        context: ErrorContext,
    },

    /// Error when processing an item
    #[error("Item processing error: {message} {context}")]
    Item {
        /// Error message
        message: String,
        /// Error context
        context: ErrorContext,
    },

    /// Error in the scheduler
    #[error("Scheduler error: {message} {context}")]
    Scheduler {
        /// Error message
        message: String,
        /// Error context
        context: ErrorContext,
    },

    /// Error in middleware
    #[error("Middleware error: {message} {context}")]
    Middleware {
        /// Error message
        message: String,
        /// Error context
        context: ErrorContext,
    },

    /// IO error
    #[error("IO error: {0}")]
    IoError(String),

    /// Serde error
    #[error("Serialization error: {0}")]
    SerdeError(String),

    /// Generic error
    #[error("{message} {context}")]
    Other {
        /// Error message
        message: String,
        /// Error context
        context: ErrorContext,
    },
}

impl Error {
    /// Create a new network error
    pub fn network(error: NetworkError) -> Self {
        Self::Network {
            error,
            context: ErrorContext::new(),
        }
    }
    
    /// Create a new HTTP error
    pub fn http(error: HttpError) -> Self {
        Self::Http {
            error,
            context: ErrorContext::new(),
        }
    }
    
    /// Create a new parse error
    pub fn parse(error: ResponseParseError) -> Self {
        Self::Parse {
            error,
            context: ErrorContext::new(),
        }
    }
    
    /// Create a new item error
    pub fn item(message: impl Into<String>) -> Self {
        Self::Item {
            message: message.into(),
            context: ErrorContext::new(),
        }
    }
    
    /// Create a new scheduler error
    pub fn scheduler(message: impl Into<String>) -> Self {
        Self::Scheduler {
            message: message.into(),
            context: ErrorContext::new(),
        }
    }
    
    /// Create a new middleware error
    pub fn middleware(message: impl Into<String>) -> Self {
        Self::Middleware {
            message: message.into(),
            context: ErrorContext::new(),
        }
    }
    
    /// Create a new generic error
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
            context: ErrorContext::new(),
        }
    }
    
    /// Get the error context
    pub fn context(&self) -> Option<&ErrorContext> {
        match self {
            Self::Network { context, .. } => Some(context),
            Self::Http { context, .. } => Some(context),
            Self::Parse { context, .. } => Some(context),
            Self::Item { context, .. } => Some(context),
            Self::Scheduler { context, .. } => Some(context),
            Self::Middleware { context, .. } => Some(context),
            Self::Other { context, .. } => Some(context),
            _ => None,
        }
    }
    
    /// Get a mutable reference to the error context
    pub fn context_mut(&mut self) -> Option<&mut ErrorContext> {
        match self {
            Self::Network { context, .. } => Some(context),
            Self::Http { context, .. } => Some(context),
            Self::Parse { context, .. } => Some(context),
            Self::Item { context, .. } => Some(context),
            Self::Scheduler { context, .. } => Some(context),
            Self::Middleware { context, .. } => Some(context),
            Self::Other { context, .. } => Some(context),
            _ => None,
        }
    }
    
    /// Set the error context
    pub fn with_context(mut self, context: ErrorContext) -> Self {
        if let Some(ctx) = self.context_mut() {
            *ctx = context;
        }
        self
    }
    
    /// Set the URL in the error context
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        if let Some(ctx) = self.context_mut() {
            ctx.url = Some(url.into());
        }
        self
    }
    
    /// Set the status code in the error context
    pub fn with_status_code(mut self, status_code: u16) -> Self {
        if let Some(ctx) = self.context_mut() {
            ctx.status_code = Some(status_code);
        }
        self
    }
    
    /// Set the request ID in the error context
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        if let Some(ctx) = self.context_mut() {
            ctx.request_id = Some(request_id.into());
        }
        self
    }
    
    /// Set the spider name in the error context
    pub fn with_spider_name(mut self, spider_name: impl Into<String>) -> Self {
        if let Some(ctx) = self.context_mut() {
            ctx.spider_name = Some(spider_name.into());
        }
        self
    }
    
    /// Set the component in the error context
    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        if let Some(ctx) = self.context_mut() {
            ctx.component = Some(component.into());
        }
        self
    }
    
    /// Add metadata to the error context
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let Some(ctx) = self.context_mut() {
            ctx.metadata.insert(key.into(), value.into());
        }
        self
    }
    
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            // Network errors are generally retryable
            Self::Network { error, .. } => match error {
                NetworkError::ConnectionRefused => true,
                NetworkError::ConnectionReset => true,
                NetworkError::ConnectionTimeout => true,
                NetworkError::DnsError(_) => true,
                NetworkError::Timeout(_) => true,
                _ => false,
            },
            
            // Some HTTP errors are retryable
            Self::Http { error, .. } => match error {
                HttpError::ServerError { status, .. } => {
                    // 5xx errors are generally retryable
                    *status >= 500 && *status < 600
                }
                HttpError::RateLimited { .. } => true,
                _ => false,
            },
            
            // Other errors are not retryable by default
            _ => false,
        }
    }
    
    /// Get the recommended retry delay for this error
    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            Self::Http { error: HttpError::RateLimited { retry_after, .. }, .. } => *retry_after,
            Self::Network { error: NetworkError::Timeout(_), .. } => Some(Duration::from_secs(5)),
            Self::Network { .. } => Some(Duration::from_secs(2)),
            Self::Http { error: HttpError::ServerError { .. }, .. } => Some(Duration::from_secs(5)),
            _ => None,
        }
    }
}

/// Result type for RS-Spider operations
pub type Result<T> = std::result::Result<T, Error>;

// Implement From for std::io::Error
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IoError(error.to_string())
    }
}

// Implement From for serde_json::Error
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::SerdeError(error.to_string())
    }
} 