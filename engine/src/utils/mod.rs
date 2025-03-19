// Utility functions for the engine

use std::sync::Arc;
use url::Url;

use scrapy_rs_core::error::Result;
use scrapy_rs_scheduler::{
    BreadthFirstScheduler, DepthFirstScheduler, DomainGroupScheduler, MemoryScheduler, Scheduler,
    SchedulerConfig,
};

use crate::config::{EngineConfig, SchedulerType};

/// Create a scheduler based on the engine configuration
pub fn create_scheduler(config: &EngineConfig) -> Result<Arc<dyn Scheduler>> {
    match config.scheduler_type {
        SchedulerType::Memory => {
            let scheduler = MemoryScheduler::new();
            Ok(Arc::new(scheduler))
        }
        SchedulerType::DomainGroup => {
            let scheduler_config = SchedulerConfig {
                strategy: config.crawl_strategy,
                max_requests_per_domain: config.max_requests_per_domain,
                domain_delay_ms: config.domain_delay_ms,
                domain_whitelist: None,
                domain_blacklist: None,
                respect_depth: true,
                max_depth: config.max_depth,
            };
            let scheduler = DomainGroupScheduler::new(scheduler_config);
            Ok(Arc::new(scheduler))
        }
        SchedulerType::BreadthFirst => {
            let scheduler = BreadthFirstScheduler::new(config.max_depth);
            Ok(Arc::new(scheduler))
        }
        SchedulerType::DepthFirst => {
            let scheduler = DepthFirstScheduler::new(config.max_depth);
            Ok(Arc::new(scheduler))
        }
    }
}

/// Extract domain from URL
pub fn extract_domain(url: &Url) -> String {
    url.host_str().unwrap_or("unknown").to_string()
}

/// Format a duration in human-readable form
pub fn format_duration(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format!("{}h {}m {}s", hours, minutes, seconds)
}
