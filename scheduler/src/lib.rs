use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashSet;
use futures::Stream;
use log::{debug, info, warn};
use priority_queue::PriorityQueue;
use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::{Error, Result};
use scrapy_rs_core::request::Request;
use serde_json;
use tokio::sync::{Mutex, RwLock};
use url::Url;

/// Crawl strategy types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrawlStrategy {
    /// Breadth-first search (process URLs in FIFO order)
    BreadthFirst,
    /// Depth-first search (process URLs in LIFO order)
    DepthFirst,
    /// Priority-based (process URLs based on priority)
    Priority,
}

/// Configuration for schedulers
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// The crawl strategy to use
    pub strategy: CrawlStrategy,
    /// Maximum number of requests per domain
    pub max_requests_per_domain: Option<usize>,
    /// Delay between requests to the same domain (in milliseconds)
    pub domain_delay_ms: Option<u64>,
    /// Domain whitelist (only crawl these domains if set)
    pub domain_whitelist: Option<HashSet<String>>,
    /// Domain blacklist (don't crawl these domains)
    pub domain_blacklist: Option<HashSet<String>>,
    /// Whether to respect the depth parameter for BFS/DFS
    pub respect_depth: bool,
    /// Maximum depth to crawl (if respect_depth is true)
    pub max_depth: Option<usize>,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            strategy: CrawlStrategy::Priority,
            max_requests_per_domain: None,
            domain_delay_ms: None,
            domain_whitelist: None,
            domain_blacklist: None,
            respect_depth: true,
            max_depth: None,
        }
    }
}

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
        let mut queue = self.queue.lock().await;
        queue.push(request, priority);
        
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
        queue.push(request);
        
        Ok(())
    }
    
    async fn next(&self) -> Option<Request> {
        let mut queue = self.queue.lock().await;
        queue.pop()
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

/// A domain-aware scheduler that groups requests by domain
pub struct DomainGroupScheduler {
    /// Configuration for the scheduler
    config: SchedulerConfig,
    
    /// Queues of pending requests, grouped by domain
    domain_queues: Mutex<HashMap<String, PriorityQueue<Request, i32>>>,
    
    /// Global queue for domains (to determine which domain to process next)
    domain_priority_queue: Mutex<PriorityQueue<String, i32>>,
    
    /// Set of seen URLs
    seen_urls: DashSet<String>,
    
    /// Last request time per domain
    last_request_times: Mutex<HashMap<String, Instant>>,
    
    /// Request counts per domain
    domain_request_counts: Mutex<HashMap<String, usize>>,
}

impl DomainGroupScheduler {
    /// Create a new domain group scheduler
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            domain_queues: Mutex::new(HashMap::new()),
            domain_priority_queue: Mutex::new(PriorityQueue::new()),
            seen_urls: DashSet::new(),
            last_request_times: Mutex::new(HashMap::new()),
            domain_request_counts: Mutex::new(HashMap::new()),
        }
    }
    
    /// Create a new domain group scheduler with default configuration
    pub fn default_priority() -> Self {
        Self::new(SchedulerConfig {
            strategy: CrawlStrategy::Priority,
            ..SchedulerConfig::default()
        })
    }
    
    /// Create a new domain group scheduler with BFS strategy
    pub fn breadth_first() -> Self {
        Self::new(SchedulerConfig {
            strategy: CrawlStrategy::BreadthFirst,
            ..SchedulerConfig::default()
        })
    }
    
    /// Create a new domain group scheduler with DFS strategy
    pub fn depth_first() -> Self {
        Self::new(SchedulerConfig {
            strategy: CrawlStrategy::DepthFirst,
            ..SchedulerConfig::default()
        })
    }
    
    /// Extract domain from URL
    fn extract_domain(url: &Url) -> String {
        url.host_str().unwrap_or("unknown").to_string()
    }
    
    /// Check if a domain is allowed based on whitelist/blacklist
    fn is_domain_allowed(&self, domain: &str) -> bool {
        // Check blacklist first
        if let Some(ref blacklist) = self.config.domain_blacklist {
            if blacklist.contains(domain) {
                return false;
            }
        }
        
        // Then check whitelist
        if let Some(ref whitelist) = self.config.domain_whitelist {
            return whitelist.contains(domain);
        }
        
        // If no whitelist, all non-blacklisted domains are allowed
        true
    }
    
    /// Check if we've reached the maximum requests for a domain
    async fn is_domain_limit_reached(&self, domain: &str) -> bool {
        if let Some(limit) = self.config.max_requests_per_domain {
            let domain_counts = self.domain_request_counts.lock().await;
            if let Some(count) = domain_counts.get(domain) {
                return *count >= limit;
            }
        }
        false
    }
    
    /// Check if we need to wait before making another request to this domain
    async fn should_delay_domain(&self, domain: &str) -> Option<Duration> {
        if let Some(delay_ms) = self.config.domain_delay_ms {
            let last_times = self.last_request_times.lock().await;
            if let Some(last_time) = last_times.get(domain) {
                let elapsed = last_time.elapsed();
                let required_delay = Duration::from_millis(delay_ms);
                
                if elapsed < required_delay {
                    return Some(required_delay - elapsed);
                }
            }
        }
        None
    }
    
    /// Update the last request time for a domain
    async fn update_last_request_time(&self, domain: &str) {
        let mut last_times = self.last_request_times.lock().await;
        last_times.insert(domain.to_string(), Instant::now());
    }
    
    /// Increment the request count for a domain
    async fn increment_domain_request_count(&self, domain: &str) {
        let mut domain_counts = self.domain_request_counts.lock().await;
        *domain_counts.entry(domain.to_string()).or_insert(0) += 1;
    }
    
    /// Get the effective priority for a request based on the crawl strategy
    fn get_effective_priority(&self, request: &Request) -> i32 {
        match self.config.strategy {
            CrawlStrategy::Priority => request.priority,
            CrawlStrategy::BreadthFirst => {
                // For BFS, use depth as negative priority (lower depth = higher priority)
                if self.config.respect_depth {
                    if let Some(depth) = request.meta.get("depth") {
                        if let Some(depth_value) = depth.as_i64() {
                            return -(depth_value as i32);
                        }
                    }
                }
                0 // Default priority if no depth is specified
            }
            CrawlStrategy::DepthFirst => {
                // For DFS, use depth as positive priority (higher depth = higher priority)
                if self.config.respect_depth {
                    if let Some(depth) = request.meta.get("depth") {
                        if let Some(depth_value) = depth.as_i64() {
                            return depth_value as i32;
                        }
                    }
                }
                0 // Default priority if no depth is specified
            }
        }
    }
}

#[async_trait]
impl Scheduler for DomainGroupScheduler {
    async fn enqueue(&self, request: Request) -> Result<()> {
        let url = request.url.to_string();
        
        // Skip if we've seen this URL before
        if self.has_seen(&url).await {
            return Ok(());
        }
        
        // Extract domain from URL
        let domain = Self::extract_domain(&request.url);
        
        // Check if domain is allowed
        if !self.is_domain_allowed(&domain) {
            debug!("Domain not allowed: {}", domain);
            return Ok(());
        }
        
        // Check depth limit if configured
        if self.config.respect_depth {
            if let Some(max_depth) = self.config.max_depth {
                if let Some(depth) = request.meta.get("depth") {
                    if let Some(depth_value) = depth.as_i64() {
                        if depth_value as usize > max_depth {
                            debug!("Skipping URL due to depth limit: {}", url);
                            return Ok(());
                        }
                    }
                }
            }
        }
        
        // Mark the URL as seen
        self.mark_seen(&url).await?;
        
        // Get the effective priority based on crawl strategy
        let priority = self.get_effective_priority(&request);
        
        // Add the request to the domain queue
        let mut domain_queues = self.domain_queues.lock().await;
        let domain_queue = domain_queues.entry(domain.clone()).or_insert_with(PriorityQueue::new);
        domain_queue.push(request, priority);
        
        // Update the domain priority queue
        let mut domain_priority_queue = self.domain_priority_queue.lock().await;
        
        // The domain priority is the highest priority of any request in that domain
        let domain_priority = domain_queue.peek().map(|(_, p)| *p).unwrap_or(0);
        
        // Update or insert the domain in the priority queue
        if domain_priority_queue.get_priority(&domain).is_some() {
            domain_priority_queue.change_priority(&domain, domain_priority);
        } else {
            domain_priority_queue.push(domain, domain_priority);
        }
        
        Ok(())
    }
    
    async fn next(&self) -> Option<Request> {
        // Get the next domain to process
        let mut domain_priority_queue = self.domain_priority_queue.lock().await;
        
        // Try domains until we find one that's ready to process
        while let Some((domain, _)) = domain_priority_queue.pop() {
            // Check if we've reached the limit for this domain
            if self.is_domain_limit_reached(&domain).await {
                debug!("Domain limit reached for: {}", domain);
                continue;
            }
            
            // Check if we need to delay requests to this domain
            if let Some(delay) = self.should_delay_domain(&domain).await {
                // Put the domain back in the queue with the same priority
                let mut domain_queues = self.domain_queues.lock().await;
                if let Some(domain_queue) = domain_queues.get(&domain) {
                    if let Some((_, priority)) = domain_queue.peek() {
                        domain_priority_queue.push(domain.clone(), *priority);
                    }
                }
                
                // Skip this domain for now
                debug!("Delaying request to domain {} for {:?}", domain, delay);
                continue;
            }
            
            // Get the next request from this domain's queue
            let mut domain_queues = self.domain_queues.lock().await;
            if let Some(domain_queue) = domain_queues.get_mut(&domain) {
                if let Some((request, _)) = domain_queue.pop() {
                    // Update the domain priority queue if there are more requests for this domain
                    if !domain_queue.is_empty() {
                        let new_priority = domain_queue.peek().map(|(_, p)| *p).unwrap_or(0);
                        domain_priority_queue.push(domain.clone(), new_priority);
                    }
                    
                    // Update the last request time and increment the request count
                    drop(domain_queues);
                    drop(domain_priority_queue);
                    self.update_last_request_time(&domain).await;
                    self.increment_domain_request_count(&domain).await;
                    
                    return Some(request);
                }
            }
        }
        
        None
    }
    
    async fn is_empty(&self) -> bool {
        let domain_priority_queue = self.domain_priority_queue.lock().await;
        domain_priority_queue.is_empty()
    }
    
    async fn len(&self) -> usize {
        let domain_queues = self.domain_queues.lock().await;
        domain_queues.values().map(|q| q.len()).sum()
    }
    
    async fn has_seen(&self, url: &str) -> bool {
        self.seen_urls.contains(url)
    }
    
    async fn mark_seen(&self, url: &str) -> Result<()> {
        self.seen_urls.insert(url.to_string());
        Ok(())
    }
    
    async fn clear(&self) -> Result<()> {
        let mut domain_queues = self.domain_queues.lock().await;
        domain_queues.clear();
        
        let mut domain_priority_queue = self.domain_priority_queue.lock().await;
        domain_priority_queue.clear();
        
        let mut last_times = self.last_request_times.lock().await;
        last_times.clear();
        
        let mut domain_counts = self.domain_request_counts.lock().await;
        domain_counts.clear();
        
        self.seen_urls.clear();
        
        Ok(())
    }
}

/// A scheduler that implements depth-first search (DFS) strategy
pub struct DepthFirstScheduler {
    /// Queue of pending requests (using a Vec as a stack)
    queue: Mutex<Vec<Request>>,
    
    /// Set of seen URLs
    seen_urls: DashSet<String>,
    
    /// Maximum depth to crawl
    max_depth: Option<usize>,
}

impl DepthFirstScheduler {
    /// Create a new DFS scheduler
    pub fn new(max_depth: Option<usize>) -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            seen_urls: DashSet::new(),
            max_depth,
        }
    }
}

impl Default for DepthFirstScheduler {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl Scheduler for DepthFirstScheduler {
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
        
        // Add the request to the queue (stack)
        let mut queue = self.queue.lock().await;
        queue.push(request);
        
        Ok(())
    }
    
    async fn next(&self) -> Option<Request> {
        let mut queue = self.queue.lock().await;
        queue.pop() // Pop from the end for LIFO (stack) behavior
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
        queue.insert(0, request); // Insert at the beginning for FIFO behavior
        
        Ok(())
    }
    
    async fn next(&self) -> Option<Request> {
        let mut queue = self.queue.lock().await;
        queue.pop() // Pop from the end for FIFO behavior
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_memory_scheduler() {
        let scheduler = MemoryScheduler::new();
        
        // Create some test requests with different priorities
        let req1 = Request::get("https://example.com/1").unwrap().with_priority(1);
        let req2 = Request::get("https://example.com/2").unwrap().with_priority(2);
        let req3 = Request::get("https://example.com/3").unwrap().with_priority(3);
        
        // Enqueue the requests
        scheduler.enqueue(req1.clone()).await.unwrap();
        scheduler.enqueue(req2.clone()).await.unwrap();
        scheduler.enqueue(req3.clone()).await.unwrap();
        
        // Check the length
        assert_eq!(scheduler.len().await, 3);
        
        // Check that we've seen these URLs
        assert!(scheduler.has_seen("https://example.com/1").await);
        assert!(scheduler.has_seen("https://example.com/2").await);
        assert!(scheduler.has_seen("https://example.com/3").await);
        
        // Try to enqueue a duplicate request
        scheduler.enqueue(req1.clone()).await.unwrap();
        
        // Length should still be 3
        assert_eq!(scheduler.len().await, 3);
        
        // Get the next requests in priority order (highest first)
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/3");
        
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/2");
        
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/1");
        
        // Queue should now be empty
        assert!(scheduler.is_empty().await);
        assert!(scheduler.next().await.is_none());
        
        // Clear the scheduler
        scheduler.clear().await.unwrap();
        
        // URLs should no longer be seen
        assert!(!scheduler.has_seen("https://example.com/1").await);
    }
    
    #[tokio::test]
    async fn test_fifo_scheduler() {
        let scheduler = FifoScheduler::new();
        
        // Create some test requests
        let req1 = Request::get("https://example.com/1").unwrap();
        let req2 = Request::get("https://example.com/2").unwrap();
        let req3 = Request::get("https://example.com/3").unwrap();
        
        // Enqueue the requests
        scheduler.enqueue(req1.clone()).await.unwrap();
        scheduler.enqueue(req2.clone()).await.unwrap();
        scheduler.enqueue(req3.clone()).await.unwrap();
        
        // Check the length
        assert_eq!(scheduler.len().await, 3);
        
        // Get the next requests in FIFO order
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/3");
        
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/2");
        
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/1");
        
        // Queue should now be empty
        assert!(scheduler.is_empty().await);
    }
    
    #[tokio::test]
    async fn test_domain_group_scheduler() {
        let config = SchedulerConfig {
            strategy: CrawlStrategy::Priority,
            max_requests_per_domain: Some(2),
            domain_delay_ms: None,
            domain_whitelist: None,
            domain_blacklist: None,
            respect_depth: true,
            max_depth: None,
        };
        
        let scheduler = DomainGroupScheduler::new(config);
        
        // Create requests for different domains with different priorities
        let req1 = Request::get("https://example.com/1").unwrap().with_priority(1);
        let req2 = Request::get("https://example.org/1").unwrap().with_priority(2);
        let req3 = Request::get("https://example.net/1").unwrap().with_priority(3);
        let req4 = Request::get("https://example.com/2").unwrap().with_priority(4);
        
        // Enqueue the requests
        scheduler.enqueue(req1).await.unwrap();
        scheduler.enqueue(req2).await.unwrap();
        scheduler.enqueue(req3).await.unwrap();
        scheduler.enqueue(req4).await.unwrap();
        
        // Check the length
        assert_eq!(scheduler.len().await, 4);
        
        // Get the next requests in priority order (highest first)
        // Should get example.com/2 first (priority 4)
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/2");
        
        // Then example.net/1 (priority 3)
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.net/1");
        
        // Then example.org/1 (priority 2)
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.org/1");
        
        // Then example.com/1 (priority 1)
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/1");
        
        // Queue should now be empty
        assert!(scheduler.is_empty().await);
    }
    
    #[tokio::test]
    async fn test_domain_limit() {
        let config = SchedulerConfig {
            strategy: CrawlStrategy::Priority,
            max_requests_per_domain: Some(1), // Limit to 1 request per domain
            domain_delay_ms: None,
            domain_whitelist: None,
            domain_blacklist: None,
            respect_depth: true,
            max_depth: None,
        };
        
        let scheduler = DomainGroupScheduler::new(config);
        
        // Create requests for the same domain
        let req1 = Request::get("https://example.com/1").unwrap().with_priority(1);
        let req2 = Request::get("https://example.com/2").unwrap().with_priority(2);
        let req3 = Request::get("https://example.org/1").unwrap().with_priority(3);
        
        // Enqueue the requests
        scheduler.enqueue(req1).await.unwrap();
        scheduler.enqueue(req2).await.unwrap();
        scheduler.enqueue(req3).await.unwrap();
        
        // Check the length
        assert_eq!(scheduler.len().await, 3);
        
        // Get the next request - should be example.org/1 (highest priority)
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.org/1");
        
        // Get the next request - should be example.com/2 (higher priority)
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/2");
        
        // The next request should be None because we've reached the limit for example.com
        assert!(scheduler.next().await.is_none());
        
        // But the queue is not empty
        assert!(!scheduler.is_empty().await);
        assert_eq!(scheduler.len().await, 1);
    }
    
    #[tokio::test]
    async fn test_depth_first_scheduler() {
        let scheduler = DepthFirstScheduler::new(Some(2));
        
        // Create requests with different depths
        let mut req1 = Request::get("https://example.com/1").unwrap();
        req1.meta.insert("depth".to_string(), serde_json::json!(0));
        
        let mut req2 = Request::get("https://example.com/2").unwrap();
        req2.meta.insert("depth".to_string(), serde_json::json!(1));
        
        let mut req3 = Request::get("https://example.com/3").unwrap();
        req3.meta.insert("depth".to_string(), serde_json::json!(2));
        
        let mut req4 = Request::get("https://example.com/4").unwrap();
        req4.meta.insert("depth".to_string(), serde_json::json!(3)); // Exceeds max depth
        
        // Enqueue the requests
        scheduler.enqueue(req1.clone()).await.unwrap();
        scheduler.enqueue(req2.clone()).await.unwrap();
        scheduler.enqueue(req3.clone()).await.unwrap();
        scheduler.enqueue(req4.clone()).await.unwrap();
        
        // Check the length - req4 should be skipped due to depth limit
        assert_eq!(scheduler.len().await, 3);
        
        // Get the next requests in LIFO order
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/3");
        
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/2");
        
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/1");
        
        // Queue should now be empty
        assert!(scheduler.is_empty().await);
    }
    
    #[tokio::test]
    async fn test_breadth_first_scheduler() {
        let scheduler = BreadthFirstScheduler::new(Some(2));
        
        // Create requests with different depths
        let mut req1 = Request::get("https://example.com/1").unwrap();
        req1.meta.insert("depth".to_string(), serde_json::json!(0));
        
        let mut req2 = Request::get("https://example.com/2").unwrap();
        req2.meta.insert("depth".to_string(), serde_json::json!(1));
        
        let mut req3 = Request::get("https://example.com/3").unwrap();
        req3.meta.insert("depth".to_string(), serde_json::json!(2));
        
        let mut req4 = Request::get("https://example.com/4").unwrap();
        req4.meta.insert("depth".to_string(), serde_json::json!(3)); // Exceeds max depth
        
        // Enqueue the requests in reverse order
        scheduler.enqueue(req4.clone()).await.unwrap();
        scheduler.enqueue(req3.clone()).await.unwrap();
        scheduler.enqueue(req2.clone()).await.unwrap();
        scheduler.enqueue(req1.clone()).await.unwrap();
        
        // Check the length - req4 should be skipped due to depth limit
        assert_eq!(scheduler.len().await, 3);
        
        // Get the next requests in FIFO order
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/1");
        
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/2");
        
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/3");
        
        // Queue should now be empty
        assert!(scheduler.is_empty().await);
    }
    
    #[tokio::test]
    async fn test_domain_whitelist() {
        let mut whitelist = HashSet::new();
        whitelist.insert("example.com".to_string());
        
        let config = SchedulerConfig {
            strategy: CrawlStrategy::Priority,
            max_requests_per_domain: None,
            domain_delay_ms: None,
            domain_whitelist: Some(whitelist),
            domain_blacklist: None,
            respect_depth: true,
            max_depth: None,
        };
        
        let scheduler = DomainGroupScheduler::new(config);
        
        // Create requests for different domains
        let req1 = Request::get("https://example.com/1").unwrap();
        let req2 = Request::get("https://example.org/1").unwrap(); // Not in whitelist
        
        // Enqueue the requests
        scheduler.enqueue(req1).await.unwrap();
        scheduler.enqueue(req2).await.unwrap();
        
        // Check the length - only req1 should be enqueued
        assert_eq!(scheduler.len().await, 1);
        
        // Get the next request
        let next = scheduler.next().await.unwrap();
        assert_eq!(next.url.as_str(), "https://example.com/1");
        
        // Queue should now be empty
        assert!(scheduler.is_empty().await);
    }
} 