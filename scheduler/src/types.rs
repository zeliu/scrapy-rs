use std::collections::HashSet;

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
