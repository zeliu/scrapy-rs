use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use log::{error, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{Error, ErrorContext, HttpError, NetworkError, ResponseParseError, Result};
use crate::request::Request;
use crate::response::Response;
use crate::spider::Spider;

/// Error handler trait for handling errors during crawling
#[async_trait]
pub trait ErrorHandler: Send + Sync + 'static {
    /// Handle an error that occurred during crawling
    async fn handle_error(&self, error: &Error, spider: &dyn Spider) -> Result<ErrorAction>;
}

/// Error action to take after handling an error
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorAction {
    /// Ignore the error and continue
    Ignore,
    
    /// Retry the request
    Retry {
        /// The request to retry
        request: Request,
        
        /// Delay before retrying (if None, use default delay)
        delay: Option<Duration>,
        
        /// Reason for retrying
        reason: String,
    },
    
    /// Abort the crawl
    Abort {
        /// Reason for aborting
        reason: String,
    },
    
    /// Skip the current item/request and continue
    Skip {
        /// Reason for skipping
        reason: String,
    },
}

impl fmt::Display for ErrorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ignore => write!(f, "Ignore"),
            Self::Retry { request, delay, reason } => {
                write!(f, "Retry {} after {:?}: {}", request.url, delay, reason)
            }
            Self::Abort { reason } => write!(f, "Abort: {}", reason),
            Self::Skip { reason } => write!(f, "Skip: {}", reason),
        }
    }
}

/// Default error handler that implements common error handling strategies
pub struct DefaultErrorHandler {
    /// Maximum number of retries
    max_retries: u32,
    
    /// Whether to retry network errors
    retry_network_errors: bool,
    
    /// Whether to retry HTTP errors
    retry_http_errors: bool,
    
    /// Whether to retry parse errors
    retry_parse_errors: bool,
    
    /// Base delay for exponential backoff (in seconds)
    base_delay: u64,
    
    /// Maximum delay for exponential backoff (in seconds)
    max_delay: u64,
    
    /// Custom error handlers for specific error types
    custom_handlers: HashMap<String, Box<dyn ErrorHandler>>,
}

impl DefaultErrorHandler {
    /// Create a new default error handler
    pub fn new() -> Self {
        Self {
            max_retries: 3,
            retry_network_errors: true,
            retry_http_errors: true,
            retry_parse_errors: false,
            base_delay: 1,
            max_delay: 60,
            custom_handlers: HashMap::new(),
        }
    }
    
    /// Set the maximum number of retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
    
    /// Set whether to retry network errors
    pub fn with_retry_network_errors(mut self, retry: bool) -> Self {
        self.retry_network_errors = retry;
        self
    }
    
    /// Set whether to retry HTTP errors
    pub fn with_retry_http_errors(mut self, retry: bool) -> Self {
        self.retry_http_errors = retry;
        self
    }
    
    /// Set whether to retry parse errors
    pub fn with_retry_parse_errors(mut self, retry: bool) -> Self {
        self.retry_parse_errors = retry;
        self
    }
    
    /// Set the base delay for exponential backoff
    pub fn with_base_delay(mut self, base_delay: u64) -> Self {
        self.base_delay = base_delay;
        self
    }
    
    /// Set the maximum delay for exponential backoff
    pub fn with_max_delay(mut self, max_delay: u64) -> Self {
        self.max_delay = max_delay;
        self
    }
    
    /// Add a custom error handler for a specific error type
    pub fn with_custom_handler<H: ErrorHandler>(mut self, error_type: &str, handler: H) -> Self {
        self.custom_handlers.insert(error_type.to_string(), Box::new(handler));
        self
    }
    
    /// Calculate the retry delay using exponential backoff
    fn calculate_retry_delay(&self, retry_count: u32) -> Duration {
        let delay = self.base_delay * 2u64.pow(retry_count);
        let delay = delay.min(self.max_delay);
        Duration::from_secs(delay)
    }
}

#[async_trait]
impl ErrorHandler for DefaultErrorHandler {
    async fn handle_error(&self, error: &Error, spider: &dyn Spider) -> Result<ErrorAction> {
        // Check if there's a custom handler for this error type
        let error_type = match error {
            Error::Network { .. } => "network",
            Error::Http { .. } => "http",
            Error::Parse { .. } => "parse",
            Error::Item { .. } => "item",
            Error::Scheduler { .. } => "scheduler",
            Error::Middleware { .. } => "middleware",
            Error::UrlParseError(_) => "url_parse",
            Error::IoError(_) => "io",
            Error::SerdeError(_) => "serde",
            Error::Other { .. } => "other",
        };
        
        if let Some(handler) = self.custom_handlers.get(error_type) {
            return handler.handle_error(error, spider).await;
        }
        
        // Get the request from the error context
        let request = if let Some(context) = error.context() {
            if let Some(url) = &context.url {
                match Request::get(url) {
                    Ok(req) => {
                        // Copy metadata from context to request
                        let mut req = req;
                        if let Some(request_id) = &context.request_id {
                            req = req.with_meta("request_id", request_id.clone());
                        }
                        
                        // Copy retry count if present
                        if let Some(retry_count) = context.metadata.get("retry_count") {
                            if let Ok(count) = retry_count.parse::<u32>() {
                                req = req.with_meta("retry_count", count);
                            }
                        }
                        
                        Some(req)
                    }
                    Err(_) => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        
        // If we don't have a request, we can't retry
        let request = match request {
            Some(req) => req,
            None => return Ok(ErrorAction::Skip { reason: "No request to retry".to_string() }),
        };
        
        // Get the retry count from the request metadata
        let retry_count = request.meta.get("retry_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        
        // Check if we've reached the maximum number of retries
        if retry_count >= self.max_retries {
            return Ok(ErrorAction::Skip { 
                reason: format!("Maximum retries ({}) reached", self.max_retries) 
            });
        }
        
        // Determine if we should retry based on the error type
        let should_retry = match error {
            Error::Network { .. } => self.retry_network_errors && error.is_retryable(),
            Error::Http { .. } => self.retry_http_errors && error.is_retryable(),
            Error::Parse { .. } => self.retry_parse_errors,
            _ => false,
        };
        
        if should_retry {
            // Create a new request with incremented retry count
            let mut request = request.clone();
            request = request.with_meta("retry_count", retry_count + 1);
            
            // Calculate the retry delay
            let delay = error.retry_delay()
                .unwrap_or_else(|| self.calculate_retry_delay(retry_count));
            
            Ok(ErrorAction::Retry {
                request,
                delay: Some(delay),
                reason: format!("Retrying after error: {}", error),
            })
        } else {
            // Skip this request
            Ok(ErrorAction::Skip { 
                reason: format!("Non-retryable error: {}", error) 
            })
        }
    }
}

impl Default for DefaultErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Error statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorStats {
    /// Total number of errors
    pub total_errors: usize,
    
    /// Number of network errors
    pub network_errors: usize,
    
    /// Number of HTTP errors
    pub http_errors: usize,
    
    /// Number of parse errors
    pub parse_errors: usize,
    
    /// Number of item errors
    pub item_errors: usize,
    
    /// Number of scheduler errors
    pub scheduler_errors: usize,
    
    /// Number of middleware errors
    pub middleware_errors: usize,
    
    /// Number of other errors
    pub other_errors: usize,
    
    /// Number of retries
    pub retries: usize,
    
    /// Number of aborts
    pub aborts: usize,
    
    /// Number of skips
    pub skips: usize,
    
    /// Number of ignored errors
    pub ignored: usize,
    
    /// Errors by status code (for HTTP errors)
    pub errors_by_status: HashMap<u16, usize>,
    
    /// Errors by domain
    pub errors_by_domain: HashMap<String, usize>,
}

impl ErrorStats {
    /// Create a new error stats instance
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Record an error
    pub fn record_error(&mut self, error: &Error) {
        self.total_errors += 1;
        
        match error {
            Error::Network { .. } => self.network_errors += 1,
            Error::Http { error, context } => {
                self.http_errors += 1;
                
                // Record status code
                if let Some(status) = context.status_code {
                    *self.errors_by_status.entry(status).or_insert(0) += 1;
                } else if let HttpError::ClientError { status, .. } = error {
                    *self.errors_by_status.entry(*status).or_insert(0) += 1;
                } else if let HttpError::ServerError { status, .. } = error {
                    *self.errors_by_status.entry(*status).or_insert(0) += 1;
                } else if let HttpError::RateLimited { status, .. } = error {
                    *self.errors_by_status.entry(*status).or_insert(0) += 1;
                } else if let HttpError::RedirectError { status, .. } = error {
                    *self.errors_by_status.entry(*status).or_insert(0) += 1;
                }
            },
            Error::Parse { .. } => self.parse_errors += 1,
            Error::Item { .. } => self.item_errors += 1,
            Error::Scheduler { .. } => self.scheduler_errors += 1,
            Error::Middleware { .. } => self.middleware_errors += 1,
            _ => self.other_errors += 1,
        }
        
        // Record domain
        if let Some(context) = error.context() {
            if let Some(url) = &context.url {
                if let Ok(parsed_url) = url::Url::parse(url) {
                    if let Some(domain) = parsed_url.host_str() {
                        *self.errors_by_domain.entry(domain.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    
    /// Record an error action
    pub fn record_action(&mut self, action: &ErrorAction) {
        match action {
            ErrorAction::Retry { .. } => self.retries += 1,
            ErrorAction::Abort { .. } => self.aborts += 1,
            ErrorAction::Skip { .. } => self.skips += 1,
            ErrorAction::Ignore => self.ignored += 1,
        }
    }
    
    /// Merge another error stats instance into this one
    pub fn merge(&mut self, other: &ErrorStats) {
        self.total_errors += other.total_errors;
        self.network_errors += other.network_errors;
        self.http_errors += other.http_errors;
        self.parse_errors += other.parse_errors;
        self.item_errors += other.item_errors;
        self.scheduler_errors += other.scheduler_errors;
        self.middleware_errors += other.middleware_errors;
        self.other_errors += other.other_errors;
        self.retries += other.retries;
        self.aborts += other.aborts;
        self.skips += other.skips;
        self.ignored += other.ignored;
        
        for (status, count) in &other.errors_by_status {
            *self.errors_by_status.entry(*status).or_insert(0) += count;
        }
        
        for (domain, count) in &other.errors_by_domain {
            *self.errors_by_domain.entry(domain.clone()).or_insert(0) += count;
        }
    }
}

/// Error manager for handling errors during crawling
pub struct ErrorManager {
    /// The error handler
    handler: Box<dyn ErrorHandler>,
    
    /// Error statistics
    stats: Arc<RwLock<ErrorStats>>,
}

impl ErrorManager {
    /// Create a new error manager with the given handler
    pub fn new<H: ErrorHandler + 'static>(handler: H) -> Self {
        Self {
            handler: Box::new(handler),
            stats: Arc::new(RwLock::new(ErrorStats::new())),
        }
    }
    
    /// Create a new error manager with the default handler
    pub fn default() -> Self {
        Self::new(DefaultErrorHandler::new())
    }
    
    /// Handle an error that occurred during crawling
    pub async fn handle_error(&self, error: &Error, spider: &dyn Spider) -> Result<ErrorAction> {
        // Record the error in stats
        {
            let mut stats = self.stats.write().await;
            stats.record_error(error);
        }
        
        // Log the error
        if let Some(context) = error.context() {
            error!("Error: {} {}", error, context);
        } else {
            error!("Error: {}", error);
        }
        
        // Handle the error
        let action = self.handler.handle_error(error, spider).await?;
        
        // Record the action in stats
        {
            let mut stats = self.stats.write().await;
            stats.record_action(&action);
        }
        
        // Log the action
        match &action {
            ErrorAction::Retry { request, delay, reason } => {
                warn!("Retrying request {} after {:?}: {}", request.url, delay, reason);
            }
            ErrorAction::Abort { reason } => {
                error!("Aborting crawl: {}", reason);
            }
            ErrorAction::Skip { reason } => {
                warn!("Skipping: {}", reason);
            }
            ErrorAction::Ignore => {
                warn!("Ignoring error");
            }
        }
        
        Ok(action)
    }
    
    /// Get the error statistics
    pub async fn stats(&self) -> ErrorStats {
        self.stats.read().await.clone()
    }
    
    /// Get a reference to the error statistics
    pub fn stats_ref(&self) -> Arc<RwLock<ErrorStats>> {
        self.stats.clone()
    }
}

/// Error callback trait for handling errors in spiders
#[async_trait]
pub trait ErrorCallback: Send + Sync + 'static {
    /// Handle an error that occurred during crawling
    async fn on_error(&self, error: &Error, response: Option<&Response>) -> Result<()>;
}

/// Error callback that logs errors
pub struct LogErrorCallback;

#[async_trait]
impl ErrorCallback for LogErrorCallback {
    async fn on_error(&self, error: &Error, response: Option<&Response>) -> Result<()> {
        if let Some(response) = response {
            error!("Error processing response from {}: {}", response.url, error);
        } else {
            error!("Error: {}", error);
        }
        Ok(())
    }
}

/// Error callback that ignores errors
pub struct IgnoreErrorCallback;

#[async_trait]
impl ErrorCallback for IgnoreErrorCallback {
    async fn on_error(&self, _error: &Error, _response: Option<&Response>) -> Result<()> {
        Ok(())
    }
}

/// Error callback that aborts the crawl on errors
pub struct AbortOnErrorCallback;

#[async_trait]
impl ErrorCallback for AbortOnErrorCallback {
    async fn on_error(&self, error: &Error, response: Option<&Response>) -> Result<()> {
        if let Some(response) = response {
            error!("Aborting crawl due to error processing response from {}: {}", response.url, error);
        } else {
            error!("Aborting crawl due to error: {}", error);
        }
        Err(Error::other("Aborting crawl due to error"))
    }
} 