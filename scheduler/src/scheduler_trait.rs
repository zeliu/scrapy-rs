use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;

/// Trait for request schedulers
#[async_trait]
pub trait Scheduler: Send + Sync + 'static {
    /// Add a request to the scheduler
    async fn enqueue(&self, request: Request) -> Result<()>;

    /// Get the next request from the scheduler
    async fn next(&self) -> Option<Request>;

    /// Check if the scheduler is empty
    async fn is_empty(&self) -> bool;

    /// Get the number of pending requests
    async fn len(&self) -> usize;

    /// Check if a URL has been seen before
    async fn has_seen(&self, url: &str) -> bool;

    /// Mark a URL as seen
    async fn mark_seen(&self, url: &str) -> Result<()>;

    /// Clear all pending requests and seen URLs
    async fn clear(&self) -> Result<()>;
}
