use crate::settings::{Result, Settings, SettingsError};
use scrapy_rs_core::spider::BasicSpider;
use scrapy_rs_downloader::{DownloaderConfig, HttpDownloader};
use scrapy_rs_engine::{EngineConfig, ResourceLimits, SchedulerType};
use scrapy_rs_scheduler::{CrawlStrategy, MemoryScheduler};
use std::sync::Arc;

/// Adapter function to create EngineConfig from Settings
pub fn engine_config_from_settings(settings: &Settings) -> Result<EngineConfig> {
    // Get scheduler type from settings
    let scheduler_type = match settings
        .get::<String>("SCHEDULER_TYPE")
        .unwrap_or_default()
        .as_str()
    {
        "domain_group" => SchedulerType::DomainGroup,
        "breadth_first" => SchedulerType::BreadthFirst,
        "depth_first" => SchedulerType::DepthFirst,
        _ => SchedulerType::Memory,
    };

    // Get crawl strategy from settings
    let crawl_strategy = match settings
        .get::<String>("CRAWL_STRATEGY")
        .unwrap_or_default()
        .as_str()
    {
        "breadth_first" => CrawlStrategy::BreadthFirst,
        "depth_first" => CrawlStrategy::DepthFirst,
        _ => CrawlStrategy::Priority,
    };

    Ok(EngineConfig {
        concurrent_requests: settings.get("CONCURRENT_REQUESTS").unwrap_or(16),
        concurrent_items: settings.get("CONCURRENT_ITEMS").unwrap_or(100),
        download_delay_ms: settings.get("DOWNLOAD_DELAY_MS").unwrap_or(0),
        max_requests_per_domain: settings.get("MAX_REQUESTS_PER_DOMAIN").ok(),
        max_requests_per_spider: settings.get("MAX_REQUESTS_PER_SPIDER").ok(),
        respect_robots_txt: settings.get("RESPECT_ROBOTS_TXT").unwrap_or(true),
        user_agent: settings
            .get("USER_AGENT")
            .unwrap_or_else(|_| format!("scrapy_rs/{}", env!("CARGO_PKG_VERSION"))),
        request_timeout: settings.get("REQUEST_TIMEOUT").unwrap_or(30),
        retry_enabled: settings.get("RETRY_ENABLED").unwrap_or(true),
        max_retries: settings.get("MAX_RETRIES").unwrap_or(3),
        follow_redirects: settings.get("FOLLOW_REDIRECTS").unwrap_or(true),
        max_depth: settings.get("MAX_DEPTH").ok(),
        log_requests: settings.get("LOG_REQUESTS").unwrap_or(true),
        log_items: settings.get("LOG_ITEMS").unwrap_or(true),
        log_stats: settings.get("LOG_STATS").unwrap_or(true),
        stats_interval_secs: settings.get("STATS_INTERVAL_SECS").unwrap_or(60),
        scheduler_type,
        crawl_strategy,
        domain_delay_ms: settings.get("DOMAIN_DELAY_MS").ok(),
        enable_resource_monitoring: settings.get("ENABLE_RESOURCE_MONITORING").unwrap_or(false),
        resource_limits: ResourceLimits {
            max_memory: settings.get("RESOURCE_MAX_MEMORY").unwrap_or(0),
            max_cpu: settings.get("RESOURCE_MAX_CPU").unwrap_or(0.0),
            max_tasks: settings.get("RESOURCE_MAX_TASKS").unwrap_or(0),
            max_pending_requests: settings.get("RESOURCE_MAX_PENDING_REQUESTS").unwrap_or(0),
            throttle_factor: settings.get("RESOURCE_THROTTLE_FACTOR").unwrap_or(0.5),
            monitor_interval_ms: settings.get("RESOURCE_MONITOR_INTERVAL_MS").unwrap_or(1000),
        },
        delay_per_domain: settings.get("DELAY_PER_DOMAIN").unwrap_or(0),
        max_active_size_per_domain: settings.get("MAX_ACTIVE_SIZE_PER_DOMAIN").unwrap_or(5000),
    })
}

/// Adapter function to create DownloaderConfig from Settings
pub fn downloader_config_from_settings(settings: &Settings) -> Result<DownloaderConfig> {
    Ok(DownloaderConfig {
        concurrent_requests: settings.get("CONCURRENT_REQUESTS").unwrap_or(10),
        user_agent: settings
            .get("USER_AGENT")
            .unwrap_or_else(|_| format!("scrapy_rs/{}", env!("CARGO_PKG_VERSION"))),
        timeout: settings.get("REQUEST_TIMEOUT").unwrap_or(30),
        retry_enabled: settings.get("RETRY_ENABLED").unwrap_or(true),
        max_retries: settings.get("MAX_RETRIES").unwrap_or(3),
        retry_delay_ms: settings.get("RETRY_DELAY_MS").unwrap_or(1000),
        max_retry_delay_ms: settings.get("MAX_RETRY_DELAY_MS").unwrap_or(30000),
        retry_backoff_factor: settings.get("RETRY_BACKOFF_FACTOR").unwrap_or(2.0),
    })
}

/// Adapter function to create BasicSpider from Settings
pub fn create_spider_from_settings(settings: &Settings) -> Result<Arc<BasicSpider>> {
    let name: String = settings
        .get("BOT_NAME")
        .unwrap_or_else(|_| "scrapy_rs".to_string());
    let start_urls: Vec<String> = settings.get("START_URLS").unwrap_or_default();
    let allowed_domains: Vec<String> = settings.get("ALLOWED_DOMAINS").unwrap_or_default();

    let mut spider = BasicSpider::new(&name, start_urls);
    if !allowed_domains.is_empty() {
        spider = spider.with_allowed_domains(allowed_domains);
    }

    // Add custom settings
    if let Ok(custom_settings) =
        settings.get::<serde_json::Map<String, serde_json::Value>>("SPIDER_SETTINGS")
    {
        for (key, value) in custom_settings {
            spider = spider.with_setting(key, value);
        }
    }

    Ok(Arc::new(spider))
}

/// Adapter function to create HttpDownloader from Settings
pub fn create_downloader_from_settings(settings: &Settings) -> Result<Arc<HttpDownloader>> {
    let config = downloader_config_from_settings(settings)?;
    let downloader = HttpDownloader::new(config)
        .map_err(|e| SettingsError::TomlParse(format!("Failed to create downloader: {}", e)))?;
    Ok(Arc::new(downloader))
}

/// Adapter function to create Scheduler from Settings
pub fn create_scheduler_from_settings(
    settings: &Settings,
) -> Result<Arc<dyn scrapy_rs_scheduler::Scheduler>> {
    // Get scheduler type from settings
    let scheduler_type = match settings
        .get::<String>("SCHEDULER_TYPE")
        .unwrap_or_default()
        .as_str()
    {
        "domain_group" => SchedulerType::DomainGroup,
        "breadth_first" => SchedulerType::BreadthFirst,
        "depth_first" => SchedulerType::DepthFirst,
        _ => SchedulerType::Memory,
    };

    // Get crawl strategy from settings
    let crawl_strategy = match settings
        .get::<String>("CRAWL_STRATEGY")
        .unwrap_or_default()
        .as_str()
    {
        "breadth_first" => CrawlStrategy::BreadthFirst,
        "depth_first" => CrawlStrategy::DepthFirst,
        _ => CrawlStrategy::Priority,
    };

    // Create scheduler based on type
    match scheduler_type {
        SchedulerType::Memory => Ok(Arc::new(MemoryScheduler::new())),
        SchedulerType::DomainGroup => {
            let config = scrapy_rs_scheduler::SchedulerConfig {
                strategy: crawl_strategy,
                max_requests_per_domain: settings.get("MAX_REQUESTS_PER_DOMAIN").ok(),
                domain_delay_ms: settings.get("DOMAIN_DELAY_MS").ok(),
                domain_whitelist: None,
                domain_blacklist: None,
                respect_depth: true,
                max_depth: settings.get("MAX_DEPTH").ok(),
            };
            Ok(Arc::new(scrapy_rs_scheduler::DomainGroupScheduler::new(
                config,
            )))
        }
        SchedulerType::BreadthFirst => {
            let max_depth = settings.get("MAX_DEPTH").ok();
            Ok(Arc::new(scrapy_rs_scheduler::BreadthFirstScheduler::new(
                max_depth,
            )))
        }
        SchedulerType::DepthFirst => {
            let max_depth = settings.get("MAX_DEPTH").ok();
            Ok(Arc::new(scrapy_rs_scheduler::DepthFirstScheduler::new(
                max_depth,
            )))
        }
    }
}
