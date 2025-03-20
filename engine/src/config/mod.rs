// Configuration types for the engine

use scrapy_rs_scheduler::CrawlStrategy;
use serde::{Deserialize, Serialize};

use crate::resource_control::ResourceLimits;

/// Scheduler type to use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulerType {
    /// Memory-based scheduler with priority queue
    Memory,
    /// Domain-aware scheduler with per-domain queues
    DomainGroup,
    /// Breadth-first search scheduler
    BreadthFirst,
    /// Depth-first search scheduler
    DepthFirst,
}

impl Default for SchedulerType {
    fn default() -> Self {
        Self::Memory
    }
}

/// Configuration for the crawler engine
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum number of concurrent requests
    pub concurrent_requests: usize,

    /// Maximum number of items to process concurrently
    pub concurrent_items: usize,

    /// Delay between requests in milliseconds
    pub download_delay_ms: u64,

    /// Maximum number of requests per domain
    pub max_requests_per_domain: Option<usize>,

    /// Maximum number of requests per spider
    pub max_requests_per_spider: Option<usize>,

    /// Whether to respect robots.txt
    pub respect_robots_txt: bool,

    /// User agent string
    pub user_agent: String,

    /// Default request timeout in seconds
    pub request_timeout: u64,

    /// Whether to retry failed requests
    pub retry_enabled: bool,

    /// Maximum number of retries
    pub max_retries: u32,

    /// Whether to follow redirects
    pub follow_redirects: bool,

    /// Maximum depth to crawl
    pub max_depth: Option<usize>,

    /// Whether to log requests and responses
    pub log_requests: bool,

    /// Whether to log items
    pub log_items: bool,

    /// Whether to log stats
    pub log_stats: bool,

    /// Interval for logging stats in seconds
    pub stats_interval_secs: u64,

    /// Type of scheduler to use
    pub scheduler_type: SchedulerType,

    /// Crawl strategy to use
    pub crawl_strategy: CrawlStrategy,

    /// Delay between requests to the same domain (in milliseconds)
    pub domain_delay_ms: Option<u64>,

    /// Resource limits
    pub resource_limits: ResourceLimits,

    /// Enable resource monitoring
    pub enable_resource_monitoring: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            concurrent_requests: 16,
            concurrent_items: 100,
            download_delay_ms: 0,
            max_requests_per_domain: None,
            max_requests_per_spider: None,
            respect_robots_txt: true,
            user_agent: format!("scrapy_rs/{}", env!("CARGO_PKG_VERSION")),
            request_timeout: 30,
            retry_enabled: true,
            max_retries: 3,
            follow_redirects: true,
            max_depth: None,
            log_requests: true,
            log_items: true,
            log_stats: true,
            stats_interval_secs: 60,
            scheduler_type: SchedulerType::Memory,
            crawl_strategy: CrawlStrategy::Priority,
            domain_delay_ms: None,
            resource_limits: ResourceLimits::default(),
            enable_resource_monitoring: false,
        }
    }
}
