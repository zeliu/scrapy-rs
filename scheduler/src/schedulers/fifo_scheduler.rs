use dashmap::DashSet;
use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use tokio::sync::Mutex;

use crate::scheduler_trait::Scheduler;

/// A FIFO scheduler that doesn't use priorities
pub struct FifoScheduler {
    /// Queue of pending requests
    queue: Mutex<Vec<Request>>,

    /// Set of seen URLs
    seen_urls: DashSet<String>,
}

impl FifoScheduler {
    /// Create a new FIFO scheduler
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            seen_urls: DashSet::new(),
        }
    }
}

impl Default for FifoScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Scheduler for FifoScheduler {
    async fn enqueue(&self, request: Request) -> Result<()> {
        let url = request.url.to_string();

        // Skip if we've seen this URL before
        if self.has_seen(&url).await {
            return Ok(());
        }

        // Mark the URL as seen
        self.mark_seen(&url).await?;

        // Add the request to the queue
        let mut queue = self.queue.lock().await;
        queue.push(request); // Add to the end for FIFO behavior

        Ok(())
    }

    async fn next(&self) -> Option<Request> {
        let mut queue = self.queue.lock().await;
        if queue.is_empty() {
            return None;
        }
        Some(queue.remove(0)) // Remove from the beginning for FIFO behavior
    }

    async fn is_empty(&self) -> bool {
        let queue = self.queue.lock().await;
        queue.is_empty()
    }

    async fn len(&self) -> usize {
        let queue = self.queue.lock().await;
        queue.len()
    }

    async fn has_seen(&self, url: &str) -> bool {
        self.seen_urls.contains(url)
    }

    async fn mark_seen(&self, url: &str) -> Result<()> {
        self.seen_urls.insert(url.to_string());
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut queue = self.queue.lock().await;
        queue.clear();
        self.seen_urls.clear();
        Ok(())
    }
}
