use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Test scenario configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestScenario {
    /// Name of the test scenario
    pub name: String,
    /// Description of the test scenario
    pub description: String,
    /// Target URLs to crawl
    pub urls: Vec<String>,
    /// Number of pages to crawl
    pub page_limit: usize,
    /// Maximum depth to crawl
    pub max_depth: usize,
    /// Concurrent requests
    pub concurrent_requests: usize,
    /// Download delay in milliseconds
    pub download_delay_ms: u64,
    /// User agent to use
    pub user_agent: String,
    /// Follow redirects
    pub follow_redirects: bool,
    /// Respect robots.txt
    pub respect_robots_txt: bool,
    /// Additional settings
    pub settings: HashMap<String, String>,
}

impl Default for TestScenario {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            description: "Default test scenario".to_string(),
            urls: vec!["https://example.com".to_string()],
            page_limit: 100,
            max_depth: 2,
            concurrent_requests: 16,
            download_delay_ms: 0,
            user_agent: "scrapy-rs-benchmark/0.1.0".to_string(),
            follow_redirects: true,
            respect_robots_txt: true,
            settings: HashMap::new(),
        }
    }
}

impl TestScenario {
    /// Create a new test scenario with the given name
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    /// Set the URLs to crawl
    pub fn with_urls(mut self, urls: Vec<String>) -> Self {
        self.urls = urls;
        self
    }

    /// Set the page limit
    pub fn with_page_limit(mut self, limit: usize) -> Self {
        self.page_limit = limit;
        self
    }

    /// Set the maximum depth
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Set the concurrent requests
    pub fn with_concurrent_requests(mut self, requests: usize) -> Self {
        self.concurrent_requests = requests;
        self
    }

    /// Set the download delay
    pub fn with_download_delay(mut self, delay_ms: u64) -> Self {
        self.download_delay_ms = delay_ms;
        self
    }

    /// Set the user agent
    pub fn with_user_agent(mut self, user_agent: &str) -> Self {
        self.user_agent = user_agent.to_string();
        self
    }

    /// Set whether to follow redirects
    pub fn with_follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = follow;
        self
    }

    /// Set whether to respect robots.txt
    pub fn with_respect_robots_txt(mut self, respect: bool) -> Self {
        self.respect_robots_txt = respect;
        self
    }

    /// Add a setting
    pub fn with_setting(mut self, key: &str, value: &str) -> Self {
        self.settings.insert(key.to_string(), value.to_string());
        self
    }
}

/// Predefined test scenarios
pub fn get_predefined_scenarios() -> Vec<TestScenario> {
    vec![
        // Simple scenario with a few pages
        TestScenario::new("simple")
            .with_description("Simple crawl of a few pages")
            .with_urls(vec!["https://example.com".to_string()])
            .with_page_limit(10)
            .with_max_depth(1)
            .with_concurrent_requests(4),
        // Medium scenario with more pages and depth
        TestScenario::new("medium")
            .with_description("Medium crawl with moderate depth")
            .with_urls(vec!["https://quotes.toscrape.com".to_string()])
            .with_page_limit(100)
            .with_max_depth(3)
            .with_concurrent_requests(8),
        // Complex scenario with multiple start URLs and high concurrency
        TestScenario::new("complex")
            .with_description("Complex crawl with multiple start URLs and high concurrency")
            .with_urls(vec![
                "https://books.toscrape.com".to_string(),
                "https://quotes.toscrape.com".to_string(),
            ])
            .with_page_limit(500)
            .with_max_depth(5)
            .with_concurrent_requests(32),
        // Real-world scenario with rate limiting
        TestScenario::new("real_world")
            .with_description("Real-world crawl with rate limiting")
            .with_urls(vec!["https://news.ycombinator.com".to_string()])
            .with_page_limit(200)
            .with_max_depth(2)
            .with_concurrent_requests(4)
            .with_download_delay(500)
            .with_user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.114 Safari/537.36"),
    ]
}

/// Item structure for benchmark tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkItem {
    /// URL of the page
    pub url: String,
    /// Title of the page
    pub title: Option<String>,
    /// Description or excerpt
    pub description: Option<String>,
    /// Links found on the page
    pub links: Vec<String>,
    /// Timestamp when the item was scraped
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl BenchmarkItem {
    /// Create a new benchmark item
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            title: None,
            description: None,
            links: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Set the title
    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// Add a link
    pub fn with_link(mut self, link: &str) -> Self {
        self.links.push(link.to_string());
        self
    }

    /// Add multiple links
    pub fn with_links(mut self, links: Vec<String>) -> Self {
        self.links.extend(links);
        self
    }
}
