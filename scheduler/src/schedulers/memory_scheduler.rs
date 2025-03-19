use dashmap::DashSet;
use priority_queue::PriorityQueue;
use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use tokio::sync::Mutex;

use crate::scheduler_trait::Scheduler;

/// A memory-based scheduler that uses a priority queue
pub struct MemoryScheduler {
    /// Queue of pending requests
    queue: Mutex<PriorityQueue<Request, i32>>,

    /// Set of seen URLs
    seen_urls: DashSet<String>,
}

impl MemoryScheduler {
    /// Create a new memory scheduler
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(PriorityQueue::new()),
            seen_urls: DashSet::new(),
        }
    }
}

impl Default for MemoryScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Scheduler for MemoryScheduler {
    async fn enqueue(&self, request: Request) -> Result<()> {
        let url = request.url.to_string();

        // Skip if we've seen this URL before
        if self.has_seen(&url).await {
            return Ok(());
        }

        // Mark the URL as seen
        self.mark_seen(&url).await?;

        // Add the request to the queue with its priority
        let priority = request.priority;
        {
            let mut queue = self.queue.lock().await;
            queue.push(request, priority);
        }

        Ok(())
    }

    async fn next(&self) -> Option<Request> {
        let mut queue = self.queue.lock().await;
        queue.pop().map(|(request, _)| request)
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
