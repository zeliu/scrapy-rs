use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use futures::future::join_all;
use log::{debug, info, warn};
use rand::Rng;
use regex::Regex;
use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::{Error, Result};
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::serde_json;
use scrapy_rs_core::spider::Spider;
use tokio::time::sleep;

/// Priority level for middleware execution
/// Higher priority middleware will be executed first
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MiddlewarePriority {
    /// Highest priority, executed first
    Highest = 1000,
    /// High priority
    High = 800,
    /// Normal priority (default)
    Normal = 500,
    /// Low priority
    Low = 200,
    /// Lowest priority, executed last
    Lowest = 0,
}

impl Default for MiddlewarePriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Trait for request middleware
#[async_trait]
pub trait RequestMiddleware: Send + Sync + 'static {
    /// Process a request before it is sent
    async fn process_request(&self, request: Request, _spider: &dyn Spider) -> Result<Request>;

    /// Called when a spider is opened
    async fn spider_opened(&self, _spider: &dyn Spider) -> Result<()> {
        Ok(())
    }

    /// Called when a spider is closed
    async fn spider_closed(&self, _spider: &dyn Spider) -> Result<()> {
        Ok(())
    }

    /// Get the priority of this middleware
    fn priority(&self) -> MiddlewarePriority;

    /// Get the name of this middleware
    fn name(&self) -> &str;

    /// Whether this middleware should be applied to the given request
    fn should_process_request(&self, _request: &Request, _spider: &dyn Spider) -> bool;
}

/// Trait for response middleware
#[async_trait]
pub trait ResponseMiddleware: Send + Sync + 'static {
    /// Process a response after it is received
    async fn process_response(&self, response: Response, _spider: &dyn Spider) -> Result<Response>;

    /// Called when a spider is opened
    async fn spider_opened(&self, _spider: &dyn Spider) -> Result<()> {
        Ok(())
    }

    /// Called when a spider is closed
    async fn spider_closed(&self, _spider: &dyn Spider) -> Result<()> {
        Ok(())
    }

    /// Get the priority of this middleware
    fn priority(&self) -> MiddlewarePriority;

    /// Get the name of this middleware
    fn name(&self) -> &str;

    /// Whether this middleware should be applied to the given response
    fn should_process_response(&self, _response: &Response, _spider: &dyn Spider) -> bool;
}

/// Log level for the ResponseLoggerMiddleware
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
}

/// A middleware that adds default headers to requests
pub struct DefaultHeadersMiddleware {
    /// Headers to add to requests
    headers: HashMap<String, String>,
    /// Priority of this middleware
    priority: MiddlewarePriority,
}

impl DefaultHeadersMiddleware {
    /// Create a new DefaultHeadersMiddleware with the given headers
    pub fn new(headers: HashMap<String, String>) -> Self {
        Self {
            headers,
            priority: MiddlewarePriority::Normal,
        }
    }

    /// Create a new DefaultHeadersMiddleware with common headers
    pub fn common() -> Self {
        let mut headers = HashMap::new();
        headers.insert("User-Agent".to_string(), "scrapy-rs/0.1.0".to_string());
        headers.insert(
            "Accept".to_string(),
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string(),
        );
        headers.insert("Accept-Language".to_string(), "en".to_string());
        Self::new(headers)
    }

    /// Add a header to the middleware
    pub fn with_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Set the priority of this middleware
    pub fn with_priority(mut self, priority: MiddlewarePriority) -> Self {
        self.priority = priority;
        self
    }
}

#[async_trait]
impl RequestMiddleware for DefaultHeadersMiddleware {
    async fn process_request(&self, mut request: Request, _spider: &dyn Spider) -> Result<Request> {
        for (key, value) in &self.headers {
            request.headers.insert(key.clone(), value.clone());
        }
        Ok(request)
    }

    fn priority(&self) -> MiddlewarePriority {
        self.priority
    }

    fn name(&self) -> &str {
        "DefaultHeadersMiddleware"
    }

    fn should_process_request(&self, _request: &Request, _spider: &dyn Spider) -> bool {
        true
    }
}

/// A middleware that adds a random delay before each request
pub struct RandomDelayMiddleware {
    /// Minimum delay in milliseconds
    min_delay_ms: u64,

    /// Maximum delay in milliseconds
    max_delay_ms: u64,
}

impl RandomDelayMiddleware {
    /// Create a new RandomDelayMiddleware with the given delay range
    pub fn new(min_delay_ms: u64, max_delay_ms: u64) -> Self {
        if min_delay_ms > max_delay_ms {
            panic!("min_delay_ms must be less than or equal to max_delay_ms");
        }
        Self {
            min_delay_ms,
            max_delay_ms,
        }
    }
}

#[async_trait]
impl RequestMiddleware for RandomDelayMiddleware {
    async fn process_request(&self, request: Request, _spider: &dyn Spider) -> Result<Request> {
        let delay = self.min_delay_ms
            + rand::thread_rng().gen_range(0..=self.max_delay_ms - self.min_delay_ms);

        debug!("Random delay middleware: sleeping for {}ms", delay);
        sleep(Duration::from_millis(delay)).await;

        Ok(request)
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    fn name(&self) -> &str {
        "RandomDelayMiddleware"
    }

    fn should_process_request(&self, _request: &Request, _spider: &dyn Spider) -> bool {
        true
    }
}

/// A middleware that filters requests based on URL patterns
pub struct UrlFilterMiddleware {
    /// Allowed URL patterns (regex)
    allowed_patterns: Vec<Regex>,

    /// Denied URL patterns (regex)
    denied_patterns: Vec<Regex>,
}

impl UrlFilterMiddleware {
    /// Create a new UrlFilterMiddleware with the given patterns
    pub fn new(allowed_patterns: Vec<Regex>, denied_patterns: Vec<Regex>) -> Self {
        Self {
            allowed_patterns,
            denied_patterns,
        }
    }

    /// Create a new UrlFilterMiddleware from string patterns
    pub fn from_strings(allowed_patterns: Vec<&str>, denied_patterns: Vec<&str>) -> Result<Self> {
        let allowed = allowed_patterns
            .into_iter()
            .map(|p| {
                Regex::new(p)
                    .map_err(|e| Box::new(Error::middleware(format!("Invalid regex: {}", e))))
            })
            .collect::<Result<Vec<_>>>()?;

        let denied = denied_patterns
            .into_iter()
            .map(|p| {
                Regex::new(p)
                    .map_err(|e| Box::new(Error::middleware(format!("Invalid regex: {}", e))))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self::new(allowed, denied))
    }
}

#[async_trait]
impl RequestMiddleware for UrlFilterMiddleware {
    async fn process_request(&self, request: Request, _spider: &dyn Spider) -> Result<Request> {
        let url_str = request.url.as_str();

        // Check denied patterns first
        for pattern in &self.denied_patterns {
            if pattern.is_match(url_str) {
                return Err(Box::new(Error::middleware(format!(
                    "URL {} matched denied pattern {}",
                    url_str, pattern
                ))));
            }
        }

        // If we have allowed patterns, at least one must match
        if !self.allowed_patterns.is_empty() {
            let mut allowed = false;
            for pattern in &self.allowed_patterns {
                if pattern.is_match(url_str) {
                    allowed = true;
                    break;
                }
            }

            if !allowed {
                return Err(Box::new(Error::middleware(format!(
                    "URL {} did not match any allowed pattern",
                    url_str
                ))));
            }
        }

        Ok(request)
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    fn name(&self) -> &str {
        "UrlFilterMiddleware"
    }

    fn should_process_request(&self, _request: &Request, _spider: &dyn Spider) -> bool {
        true
    }
}

/// A middleware that logs responses
pub struct ResponseLoggerMiddleware {
    /// Log level to use
    level: LogLevel,
}

impl ResponseLoggerMiddleware {
    /// Create a new ResponseLoggerMiddleware with the given log level
    pub fn new(level: LogLevel) -> Self {
        Self { level }
    }

    /// Create a new ResponseLoggerMiddleware with INFO level
    pub fn info() -> Self {
        Self::new(LogLevel::Info)
    }

    /// Create a new ResponseLoggerMiddleware with DEBUG level
    pub fn debug() -> Self {
        Self::new(LogLevel::Debug)
    }
}

#[async_trait]
impl ResponseMiddleware for ResponseLoggerMiddleware {
    async fn process_response(&self, response: Response, _spider: &dyn Spider) -> Result<Response> {
        match self.level {
            LogLevel::Debug => debug!("Response: {} ({})", response.url, response.status),
            LogLevel::Info => info!("Response: {} ({})", response.url, response.status),
            LogLevel::Warn => warn!("Response: {} ({})", response.url, response.status),
        }

        Ok(response)
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    fn name(&self) -> &str {
        "ResponseLoggerMiddleware"
    }

    fn should_process_response(&self, _response: &Response, _spider: &dyn Spider) -> bool {
        true
    }
}

/// A middleware that retries failed responses
pub struct RetryMiddleware {
    /// Status codes to retry
    retry_status_codes: Vec<u16>,

    /// Maximum number of retries
    max_retries: u32,

    /// Delay between retries in milliseconds
    retry_delay_ms: u64,
}

impl RetryMiddleware {
    /// Create a new RetryMiddleware with the given parameters
    pub fn new(retry_status_codes: Vec<u16>, max_retries: u32, retry_delay_ms: u64) -> Self {
        Self {
            retry_status_codes,
            max_retries,
            retry_delay_ms,
        }
    }

    /// Create a new RetryMiddleware with common settings
    pub fn common() -> Self {
        Self::new(vec![500, 502, 503, 504, 408, 429], 3, 1000)
    }
}

#[async_trait]
impl ResponseMiddleware for RetryMiddleware {
    async fn process_response(&self, response: Response, _spider: &dyn Spider) -> Result<Response> {
        // Check if we should retry this response
        let status = response.status;
        let should_retry = self.retry_status_codes.contains(&status);

        if !should_retry {
            return Ok(response);
        }

        // Get the retry count from the response meta
        let retry_count = response
            .meta
            .get("retry_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        // Check if we've reached the maximum number of retries
        if retry_count >= self.max_retries as u64 {
            warn!(
                "Maximum retries reached for {} (status {})",
                response.url, status
            );
            return Ok(response);
        }

        // Get the original request from the response
        let request = response.request.clone();

        // Increment the retry count
        info!(
            "Retrying {} (status {}), attempt {}/{}",
            request.url,
            response.status,
            retry_count + 1,
            self.max_retries
        );

        // Add the request back to the meta of the response
        let mut response = response;
        response.meta.insert(
            "retry_request".to_string(),
            serde_json::to_value(request).unwrap(),
        );

        // Sleep before returning
        sleep(Duration::from_millis(self.retry_delay_ms)).await;

        Ok(response)
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    fn name(&self) -> &str {
        "RetryMiddleware"
    }

    fn should_process_response(&self, _response: &Response, _spider: &dyn Spider) -> bool {
        true
    }
}

/// A middleware wrapper that contains a middleware and its priority
struct MiddlewareWrapper<T: ?Sized> {
    middleware: Box<T>,
    priority: MiddlewarePriority,
    name: String,
}

impl<T: ?Sized> MiddlewareWrapper<T> {
    #[allow(dead_code)]
    fn new<M>(middleware: M) -> Self
    where
        M: 'static + Into<Box<T>>,
    {
        let boxed = middleware.into();
        Self {
            middleware: boxed,
            priority: MiddlewarePriority::Normal,
            name: String::from("Unknown"),
        }
    }
}

impl<T: ?Sized> PartialEq for MiddlewareWrapper<T> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl<T: ?Sized> Eq for MiddlewareWrapper<T> {}

impl<T: ?Sized> PartialOrd for MiddlewareWrapper<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: ?Sized> Ord for MiddlewareWrapper<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority comes first (reverse ordering)
        other.priority.cmp(&self.priority)
    }
}

/// A middleware that chains multiple request middlewares together
pub struct ChainedRequestMiddleware {
    /// The middlewares to chain
    middlewares: Vec<MiddlewareWrapper<dyn RequestMiddleware>>,
    /// Whether the middlewares are sorted
    sorted: bool,
}

impl ChainedRequestMiddleware {
    /// Create a new ChainedRequestMiddleware with the given middlewares
    pub fn new(middlewares: Vec<Box<dyn RequestMiddleware>>) -> Self {
        let wrappers = middlewares
            .into_iter()
            .map(|m| {
                let priority = m.priority();
                let name = m.name().to_string();
                MiddlewareWrapper {
                    middleware: m,
                    priority,
                    name,
                }
            })
            .collect();

        Self {
            middlewares: wrappers,
            sorted: false,
        }
    }

    /// Add a middleware to the chain
    pub fn add<M: RequestMiddleware + 'static>(&mut self, middleware: M) -> &mut Self {
        let priority = middleware.priority();
        let name = middleware.name().to_string();

        self.middlewares.push(MiddlewareWrapper {
            middleware: Box::new(middleware),
            priority,
            name,
        });

        // Sort immediately after adding
        self.middlewares.sort();
        self.sorted = true;

        self
    }

    /// Sort the middlewares by priority
    fn ensure_sorted(&mut self) {
        if !self.sorted {
            self.middlewares.sort();
            self.sorted = true;

            // Log the middleware order
            if log::log_enabled!(log::Level::Debug) {
                let middleware_names: Vec<String> = self
                    .middlewares
                    .iter()
                    .map(|w| format!("{}({})", w.name, w.priority as u32))
                    .collect();
                debug!("Request middleware order: {}", middleware_names.join(", "));
            }
        }
    }

    /// Get the middlewares in priority order
    #[allow(dead_code)]
    fn get_middlewares(&mut self) -> &[MiddlewareWrapper<dyn RequestMiddleware>] {
        self.ensure_sorted();
        &self.middlewares
    }
}

#[async_trait]
impl RequestMiddleware for ChainedRequestMiddleware {
    async fn process_request(&self, mut request: Request, spider: &dyn Spider) -> Result<Request> {
        println!(
            "ChainedRequestMiddleware::process_request - Middlewares count: {}",
            self.middlewares.len()
        );

        // Use already sorted middleware list
        let middlewares = if self.sorted {
            &self.middlewares
        } else {
            // If middlewares are not sorted, we cannot sort them here (because self is immutable)
            // This is a design issue, sorting should be done when adding middlewares
            // In practice, ensure sorting is done before calling process_request
            &self.middlewares
        };

        for (i, wrapper) in middlewares.iter().enumerate() {
            println!(
                "Processing middleware {}: {} (priority: {:?})",
                i, wrapper.name, wrapper.priority
            );
            if wrapper.middleware.should_process_request(&request, spider) {
                request = wrapper.middleware.process_request(request, spider).await?;
            }
        }

        Ok(request)
    }

    async fn spider_opened(&self, spider: &dyn Spider) -> Result<()> {
        let futures = self
            .middlewares
            .iter()
            .map(|wrapper| wrapper.middleware.spider_opened(spider));

        let results = join_all(futures).await;

        for result in results {
            result?;
        }

        Ok(())
    }

    async fn spider_closed(&self, spider: &dyn Spider) -> Result<()> {
        let futures = self
            .middlewares
            .iter()
            .map(|wrapper| wrapper.middleware.spider_closed(spider));

        let results = join_all(futures).await;

        for result in results {
            result?;
        }

        Ok(())
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    fn name(&self) -> &str {
        "ChainedRequestMiddleware"
    }

    fn should_process_request(&self, _request: &Request, _spider: &dyn Spider) -> bool {
        true
    }
}

impl Clone for ChainedRequestMiddleware {
    fn clone(&self) -> Self {
        // We can't actually clone the middlewares, so we create a new empty chain
        // This is only used internally for sorting
        Self {
            middlewares: Vec::new(),
            sorted: false,
        }
    }
}

/// A middleware that chains multiple response middlewares together
pub struct ChainedResponseMiddleware {
    /// The middlewares to chain
    middlewares: Vec<MiddlewareWrapper<dyn ResponseMiddleware>>,
    /// Whether the middlewares are sorted
    sorted: bool,
}

impl ChainedResponseMiddleware {
    /// Create a new ChainedResponseMiddleware with the given middlewares
    pub fn new(middlewares: Vec<Box<dyn ResponseMiddleware>>) -> Self {
        let wrappers = middlewares
            .into_iter()
            .map(|m| {
                let priority = m.priority();
                let name = m.name().to_string();
                MiddlewareWrapper {
                    middleware: m,
                    priority,
                    name,
                }
            })
            .collect();

        Self {
            middlewares: wrappers,
            sorted: false,
        }
    }

    /// Add a middleware to the chain
    pub fn add<M: ResponseMiddleware + 'static>(&mut self, middleware: M) -> &mut Self {
        let priority = middleware.priority();
        let name = middleware.name().to_string();

        self.middlewares.push(MiddlewareWrapper {
            middleware: Box::new(middleware),
            priority,
            name,
        });

        // Sort immediately after adding
        self.middlewares.sort();
        self.sorted = true;

        self
    }

    /// Sort the middlewares by priority
    fn ensure_sorted(&mut self) {
        if !self.sorted {
            self.middlewares.sort();
            self.sorted = true;

            // Log the middleware order
            if log::log_enabled!(log::Level::Debug) {
                let middleware_names: Vec<String> = self
                    .middlewares
                    .iter()
                    .map(|w| format!("{}({})", w.name, w.priority as u32))
                    .collect();
                debug!("Response middleware order: {}", middleware_names.join(", "));
            }
        }
    }

    /// Get the middlewares in priority order
    #[allow(dead_code)]
    fn get_middlewares(&mut self) -> &[MiddlewareWrapper<dyn ResponseMiddleware>] {
        self.ensure_sorted();
        &self.middlewares
    }
}

#[async_trait]
impl ResponseMiddleware for ChainedResponseMiddleware {
    async fn process_response(
        &self,
        mut response: Response,
        spider: &dyn Spider,
    ) -> Result<Response> {
        println!(
            "ChainedResponseMiddleware::process_response - Middlewares count: {}",
            self.middlewares.len()
        );

        // Use already sorted middleware list
        let middlewares = if self.sorted {
            &self.middlewares
        } else {
            // If middlewares are not sorted, we cannot sort them here (because self is immutable)
            // This is a design issue, sorting should be done when adding middlewares
            // In practice, ensure sorting is done before calling process_response
            &self.middlewares
        };

        for (i, wrapper) in middlewares.iter().enumerate() {
            println!(
                "Processing middleware {}: {} (priority: {:?})",
                i, wrapper.name, wrapper.priority
            );
            if wrapper
                .middleware
                .should_process_response(&response, spider)
            {
                response = wrapper
                    .middleware
                    .process_response(response, spider)
                    .await?;
            }
        }

        Ok(response)
    }

    async fn spider_opened(&self, spider: &dyn Spider) -> Result<()> {
        let futures = self
            .middlewares
            .iter()
            .map(|wrapper| wrapper.middleware.spider_opened(spider));

        let results = join_all(futures).await;

        for result in results {
            result?;
        }

        Ok(())
    }

    async fn spider_closed(&self, spider: &dyn Spider) -> Result<()> {
        let futures = self
            .middlewares
            .iter()
            .map(|wrapper| wrapper.middleware.spider_closed(spider));

        let results = join_all(futures).await;

        for result in results {
            result?;
        }

        Ok(())
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    fn name(&self) -> &str {
        "ChainedResponseMiddleware"
    }

    fn should_process_response(&self, _response: &Response, _spider: &dyn Spider) -> bool {
        true
    }
}

impl Clone for ChainedResponseMiddleware {
    fn clone(&self) -> Self {
        // We can't actually clone the middlewares, so we create a new empty chain
        // This is only used internally for sorting
        Self {
            middlewares: Vec::new(),
            sorted: false,
        }
    }
}

/// A middleware that conditionally applies another middleware based on a predicate
pub struct ConditionalMiddleware<M, F>
where
    M: RequestMiddleware,
    F: Fn(&Request, &dyn Spider) -> bool + Send + Sync + 'static,
{
    /// The middleware to conditionally apply
    middleware: Arc<M>,
    /// The predicate function
    predicate: F,
    /// Priority of this middleware
    priority: MiddlewarePriority,
}

impl<M, F> ConditionalMiddleware<M, F>
where
    M: RequestMiddleware,
    F: Fn(&Request, &dyn Spider) -> bool + Send + Sync + 'static,
{
    /// Create a new ConditionalMiddleware with the given middleware and predicate
    pub fn new(middleware: M, predicate: F) -> Self {
        Self {
            middleware: Arc::new(middleware),
            predicate,
            priority: MiddlewarePriority::Normal,
        }
    }

    /// Set the priority of this middleware
    pub fn with_priority(mut self, priority: MiddlewarePriority) -> Self {
        self.priority = priority;
        self
    }
}

#[async_trait]
impl<M, F> RequestMiddleware for ConditionalMiddleware<M, F>
where
    M: RequestMiddleware,
    F: Fn(&Request, &dyn Spider) -> bool + Send + Sync + 'static,
{
    async fn process_request(&self, request: Request, spider: &dyn Spider) -> Result<Request> {
        if (self.predicate)(&request, spider) {
            self.middleware.process_request(request, spider).await
        } else {
            Ok(request)
        }
    }

    async fn spider_opened(&self, spider: &dyn Spider) -> Result<()> {
        self.middleware.spider_opened(spider).await
    }

    async fn spider_closed(&self, spider: &dyn Spider) -> Result<()> {
        self.middleware.spider_closed(spider).await
    }

    fn priority(&self) -> MiddlewarePriority {
        self.priority
    }

    fn name(&self) -> &str {
        "ConditionalMiddleware"
    }

    fn should_process_request(&self, request: &Request, spider: &dyn Spider) -> bool {
        (self.predicate)(request, spider)
    }
}

/// A middleware that conditionally applies another middleware based on a predicate
pub struct ConditionalResponseMiddleware<M, F>
where
    M: ResponseMiddleware,
    F: Fn(&Response, &dyn Spider) -> bool + Send + Sync + 'static,
{
    /// The middleware to conditionally apply
    middleware: Arc<M>,
    /// The predicate function
    predicate: F,
    /// Priority of this middleware
    priority: MiddlewarePriority,
}

impl<M, F> ConditionalResponseMiddleware<M, F>
where
    M: ResponseMiddleware,
    F: Fn(&Response, &dyn Spider) -> bool + Send + Sync + 'static,
{
    /// Create a new ConditionalResponseMiddleware with the given middleware and predicate
    pub fn new(middleware: M, predicate: F) -> Self {
        Self {
            middleware: Arc::new(middleware),
            predicate,
            priority: MiddlewarePriority::Normal,
        }
    }

    /// Set the priority of this middleware
    pub fn with_priority(mut self, priority: MiddlewarePriority) -> Self {
        self.priority = priority;
        self
    }
}

#[async_trait]
impl<M, F> ResponseMiddleware for ConditionalResponseMiddleware<M, F>
where
    M: ResponseMiddleware,
    F: Fn(&Response, &dyn Spider) -> bool + Send + Sync + 'static,
{
    async fn process_response(&self, response: Response, spider: &dyn Spider) -> Result<Response> {
        if (self.predicate)(&response, spider) {
            self.middleware.process_response(response, spider).await
        } else {
            Ok(response)
        }
    }

    async fn spider_opened(&self, spider: &dyn Spider) -> Result<()> {
        self.middleware.spider_opened(spider).await
    }

    async fn spider_closed(&self, spider: &dyn Spider) -> Result<()> {
        self.middleware.spider_closed(spider).await
    }

    fn priority(&self) -> MiddlewarePriority {
        self.priority
    }

    fn name(&self) -> &str {
        "ConditionalResponseMiddleware"
    }

    fn should_process_response(&self, response: &Response, spider: &dyn Spider) -> bool {
        (self.predicate)(response, spider)
    }
}

/// A middleware that applies a rate limit to requests
pub struct RateLimitMiddleware {
    /// Maximum number of requests per time window
    max_requests: usize,
    /// Time window in seconds
    time_window_secs: u64,
    /// Request timestamps
    timestamps: Arc<tokio::sync::Mutex<Vec<Instant>>>,
    /// Priority of this middleware
    priority: MiddlewarePriority,
}

impl RateLimitMiddleware {
    /// Create a new RateLimitMiddleware with the given rate limit
    pub fn new(max_requests: usize, time_window_secs: u64) -> Self {
        Self {
            max_requests,
            time_window_secs,
            timestamps: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            priority: MiddlewarePriority::High,
        }
    }

    /// Set the priority of this middleware
    pub fn with_priority(mut self, priority: MiddlewarePriority) -> Self {
        self.priority = priority;
        self
    }
}

#[async_trait]
impl RequestMiddleware for RateLimitMiddleware {
    async fn process_request(&self, request: Request, _spider: &dyn Spider) -> Result<Request> {
        let now = Instant::now();
        let window_duration = Duration::from_secs(self.time_window_secs);

        // Clean up old timestamps and check if we need to wait
        let mut timestamps = self.timestamps.lock().await;

        // Remove timestamps older than the time window
        timestamps.retain(|t| now.duration_since(*t) < window_duration);

        // If we've reached the maximum number of requests, wait until we can make another
        if timestamps.len() >= self.max_requests {
            let oldest = timestamps[0];
            let time_passed = now.duration_since(oldest);

            if time_passed < window_duration {
                let wait_time = window_duration - time_passed;
                debug!("Rate limit reached, waiting for {:?}", wait_time);
                sleep(wait_time).await;
            }

            // Remove the oldest timestamp
            timestamps.remove(0);
        }

        // Add the current timestamp
        timestamps.push(Instant::now());

        Ok(request)
    }

    fn priority(&self) -> MiddlewarePriority {
        self.priority
    }

    fn name(&self) -> &str {
        "RateLimitMiddleware"
    }

    fn should_process_request(&self, _request: &Request, _spider: &dyn Spider) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scrapy_rs_core::spider::BasicSpider;

    #[tokio::test]
    async fn test_default_headers_middleware() {
        let middleware = DefaultHeadersMiddleware::new(HashMap::from([(
            "X-Test".to_string(),
            "test".to_string(),
        )]));

        let request = Request::get("https://example.com").unwrap();
        let processed = middleware
            .process_request(request, &TestSpider)
            .await
            .unwrap();

        assert_eq!(processed.headers.get("X-Test").unwrap(), "test");
    }

    #[tokio::test]
    async fn test_url_filter_middleware() {
        let middleware = UrlFilterMiddleware::from_strings(
            vec!["https://example.com/.*"],
            vec!["https://example.com/forbidden"],
        )
        .unwrap();

        // Allowed URL
        let request = Request::get("https://example.com/allowed").unwrap();
        let result = middleware.process_request(request, &TestSpider).await;
        assert!(result.is_ok());

        // Denied URL
        let request = Request::get("https://example.com/forbidden").unwrap();
        let result = middleware.process_request(request, &TestSpider).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_response_logger_middleware() {
        let middleware = ResponseLoggerMiddleware::info();

        let response = Response {
            url: "https://example.com".parse().unwrap(),
            status: 200,
            headers: HashMap::new(),
            body: vec![],
            request: Request::get("https://example.com").unwrap(),
            meta: HashMap::new(),
            flags: Vec::new(),
            certificate: None,
            ip_address: None,
            protocol: None,
        };

        let processed = middleware
            .process_response(response, &TestSpider)
            .await
            .unwrap();
        assert_eq!(processed.status, 200);
    }

    #[tokio::test]
    async fn test_retry_middleware() {
        let middleware = RetryMiddleware::new(vec![500], 3, 100);

        // Create a response with a 500 status code
        let response = Response {
            url: "https://example.com".parse().unwrap(),
            status: 500,
            headers: HashMap::new(),
            body: vec![],
            request: Request::get("https://example.com").unwrap(),
            meta: HashMap::new(),
            flags: Vec::new(),
            certificate: None,
            ip_address: None,
            protocol: None,
        };

        // First retry
        let processed = middleware
            .process_response(response.clone(), &TestSpider)
            .await
            .unwrap();
        assert!(processed.meta.contains_key("retry_request"));

        // Second retry (with retry_count = 1)
        let mut response_with_count = response.clone();
        response_with_count
            .meta
            .insert("retry_count".to_string(), serde_json::json!(1));
        let processed = middleware
            .process_response(response_with_count, &TestSpider)
            .await
            .unwrap();
        assert!(processed.meta.contains_key("retry_request"));

        // Third retry (with retry_count = 2)
        let mut response_with_count = response.clone();
        response_with_count
            .meta
            .insert("retry_count".to_string(), serde_json::json!(2));
        let processed = middleware
            .process_response(response_with_count, &TestSpider)
            .await
            .unwrap();
        assert!(processed.meta.contains_key("retry_request"));

        // Fourth retry (with retry_count = 3) - should not retry
        let mut response_with_count = response.clone();
        response_with_count
            .meta
            .insert("retry_count".to_string(), serde_json::json!(3));
        let processed = middleware
            .process_response(response_with_count, &TestSpider)
            .await
            .unwrap();
        assert!(!processed.meta.contains_key("retry_request"));
    }

    #[tokio::test]
    async fn test_chained_middleware() {
        let mut chained = ChainedRequestMiddleware::new(Vec::new());

        let headers_middleware = DefaultHeadersMiddleware::new(HashMap::from([(
            "X-Test".to_string(),
            "test".to_string(),
        )]));

        chained.add(headers_middleware);

        let request = Request::get("https://example.com").unwrap();
        let processed = chained.process_request(request, &TestSpider).await.unwrap();

        assert_eq!(processed.headers.get("X-Test").unwrap(), "test");
    }

    // A dummy spider for testing
    struct TestSpider;
    use scrapy_rs_core::spider::ParseOutput;

    #[async_trait]
    impl Spider for TestSpider {
        fn name(&self) -> &str {
            "test_spider"
        }

        fn start_urls(&self) -> Vec<String> {
            vec!["https://example.com".to_string()]
        }

        async fn parse(&self, _response: Response) -> Result<ParseOutput> {
            Ok(ParseOutput::default())
        }
    }

    // Add new tests directly here
    #[tokio::test]
    async fn test_middleware_priority() {
        // Create middlewares with different priorities
        let high_priority =
            DefaultHeadersMiddleware::new(HashMap::new()).with_priority(MiddlewarePriority::High);

        let normal_priority = DefaultHeadersMiddleware::new(HashMap::new());

        let low_priority =
            DefaultHeadersMiddleware::new(HashMap::new()).with_priority(MiddlewarePriority::Low);

        // Add them in reverse order
        let mut chain = ChainedRequestMiddleware::new(vec![]);
        chain.add(low_priority);
        chain.add(normal_priority);
        chain.add(high_priority);

        // Get middlewares - they should be sorted by priority (highest first)
        let middlewares = chain.get_middlewares();

        assert_eq!(middlewares[0].priority, MiddlewarePriority::High);
        assert_eq!(middlewares[1].priority, MiddlewarePriority::Normal);
        assert_eq!(middlewares[2].priority, MiddlewarePriority::Low);
    }

    #[tokio::test]
    async fn test_conditional_middleware() {
        // Create a middleware that only processes requests to example.com
        let headers_middleware = DefaultHeadersMiddleware::common();
        let conditional = ConditionalMiddleware::new(headers_middleware, |req, _| {
            req.url.host_str() == Some("example.com")
        });

        // Create test requests
        let example_request = scrapy_rs_core::request::Request::get("https://example.com").unwrap();
        let other_request = scrapy_rs_core::request::Request::get("https://other.com").unwrap();

        let spider = BasicSpider::new("test_spider", vec!["https://example.com".to_string()]);

        // The middleware should process the example.com request
        assert!(conditional.should_process_request(&example_request, &spider));

        // The middleware should not process the other.com request
        assert!(!conditional.should_process_request(&other_request, &spider));

        // Test actual processing
        let processed_example = conditional
            .process_request(example_request.clone(), &spider)
            .await
            .unwrap();
        let processed_other = conditional
            .process_request(other_request.clone(), &spider)
            .await
            .unwrap();

        // The example.com request should have been modified (headers added)
        assert!(processed_example.headers.len() > example_request.headers.len());

        // The other.com request should remain unchanged
        assert_eq!(processed_other.headers.len(), other_request.headers.len());
    }

    #[tokio::test]
    async fn test_rate_limit_middleware() {
        // Create a rate limit middleware that allows 3 requests per second
        let rate_limit = RateLimitMiddleware::new(3, 1);

        let request = scrapy_rs_core::request::Request::get("https://example.com").unwrap();
        let spider = BasicSpider::new("test_spider", vec!["https://example.com".to_string()]);

        // First 3 requests should pass through immediately
        for _ in 0..3 {
            let start = std::time::Instant::now();
            let _ = rate_limit
                .process_request(request.clone(), &spider)
                .await
                .unwrap();
            let elapsed = start.elapsed();

            // Should be very quick
            assert!(elapsed < std::time::Duration::from_millis(50));
        }

        // The 4th request should be delayed
        let start = std::time::Instant::now();
        let _ = rate_limit
            .process_request(request.clone(), &spider)
            .await
            .unwrap();
        let elapsed = start.elapsed();

        // Should be delayed close to 1 second (the time window)
        // We use a slightly lower threshold to account for timing variations
        assert!(elapsed > std::time::Duration::from_millis(900));
    }

    #[tokio::test]
    async fn test_middleware_chain_execution_order() {
        // Create a struct to track execution order
        struct OrderTracker {
            order: Arc<tokio::sync::Mutex<Vec<String>>>,
        }

        struct TrackingMiddleware {
            name: String,
            tracker: Arc<OrderTracker>,
            priority: MiddlewarePriority,
        }

        #[async_trait::async_trait]
        impl RequestMiddleware for TrackingMiddleware {
            async fn process_request(
                &self,
                request: scrapy_rs_core::request::Request,
                _spider: &dyn scrapy_rs_core::spider::Spider,
            ) -> scrapy_rs_core::error::Result<scrapy_rs_core::request::Request> {
                let mut order = self.tracker.order.lock().await;
                order.push(self.name.clone());
                Ok(request)
            }

            fn priority(&self) -> MiddlewarePriority {
                self.priority
            }

            fn name(&self) -> &str {
                &self.name
            }

            fn should_process_request(
                &self,
                _request: &scrapy_rs_core::request::Request,
                _spider: &dyn scrapy_rs_core::spider::Spider,
            ) -> bool {
                true
            }
        }

        // Create tracker and middlewares
        let tracker = Arc::new(OrderTracker {
            order: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        });

        let high = TrackingMiddleware {
            name: "high".to_string(),
            tracker: tracker.clone(),
            priority: MiddlewarePriority::High,
        };

        let normal = TrackingMiddleware {
            name: "normal".to_string(),
            tracker: tracker.clone(),
            priority: MiddlewarePriority::Normal,
        };

        let low = TrackingMiddleware {
            name: "low".to_string(),
            tracker: tracker.clone(),
            priority: MiddlewarePriority::Low,
        };

        // Create chain and add middlewares in random order
        let mut chain = ChainedRequestMiddleware::new(vec![]);
        chain.add(normal);
        chain.add(low);
        chain.add(high);

        // Process a request
        let request = scrapy_rs_core::request::Request::get("https://example.com").unwrap();
        let spider = BasicSpider::new("test_spider", vec!["https://example.com".to_string()]);

        let _ = chain.process_request(request, &spider).await.unwrap();

        // Check execution order
        let order = tracker.order.lock().await;
        assert_eq!(*order, vec!["high", "normal", "low"]);
    }
}
