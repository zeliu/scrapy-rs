use dashmap::DashSet;
use log::debug;
use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use tokio::sync::Mutex;

use crate::scheduler_trait::Scheduler;

/// A scheduler that implements breadth-first search (BFS) strategy
pub struct BreadthFirstScheduler {
    /// Queue of pending requests
    queue: Mutex<Vec<Request>>,

    /// Set of seen URLs
    seen_urls: DashSet<String>,

    /// Maximum depth to crawl
    max_depth: Option<usize>,
}

impl BreadthFirstScheduler {
    /// Create a new BFS scheduler
    pub fn new(max_depth: Option<usize>) -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            seen_urls: DashSet::new(),
            max_depth,
        }
    }
}

impl Default for BreadthFirstScheduler {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl Scheduler for BreadthFirstScheduler {
    async fn enqueue(&self, request: Request) -> Result<()> {
        let url = request.url.to_string();

        // Skip if we've seen this URL before
        if self.has_seen(&url).await {
            return Ok(());
        }

        // Check depth limit if configured
        if let Some(max_depth) = self.max_depth {
            if let Some(depth) = request.meta.get("depth") {
                if let Some(depth_value) = depth.as_i64() {
                    if depth_value as usize > max_depth {
                        debug!("Skipping URL due to depth limit: {}", url);
                        return Ok(());
                    }
                }
            }
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
