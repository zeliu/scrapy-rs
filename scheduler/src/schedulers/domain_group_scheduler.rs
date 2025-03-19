use std::collections::HashMap;
use std::time::{Duration, Instant};

use dashmap::DashSet;
use log::debug;
use priority_queue::PriorityQueue;
use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use tokio::sync::Mutex;
use url::Url;

use crate::scheduler_trait::Scheduler;
use crate::types::SchedulerConfig;

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
            strategy: crate::types::CrawlStrategy::Priority,
            ..SchedulerConfig::default()
        })
    }

    /// Create a new domain group scheduler with BFS strategy
    pub fn breadth_first() -> Self {
        Self::new(SchedulerConfig {
            strategy: crate::types::CrawlStrategy::BreadthFirst,
            ..SchedulerConfig::default()
        })
    }

    /// Create a new domain group scheduler with DFS strategy
    pub fn depth_first() -> Self {
        Self::new(SchedulerConfig {
            strategy: crate::types::CrawlStrategy::DepthFirst,
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
            crate::types::CrawlStrategy::Priority => request.priority,
            crate::types::CrawlStrategy::BreadthFirst => {
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
            crate::types::CrawlStrategy::DepthFirst => {
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
        let domain_queue = domain_queues
            .entry(domain.clone())
            .or_insert_with(PriorityQueue::new);
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
                let domain_queues = self.domain_queues.lock().await;
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
        let domain_queues = self.domain_queues.lock().await;
        let total_requests = domain_queues.values().map(|q| q.len()).sum::<usize>();
        total_requests == 0
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
