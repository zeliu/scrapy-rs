use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use scrapy_rs_engine::EngineStats;

/// Resource statistics (simplified version that can be serialized)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStatsDto {
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// CPU usage percentage (0-100)
    pub cpu_usage: f32,
    /// Number of active tasks
    pub active_tasks: usize,
    /// Number of pending requests
    pub pending_requests: usize,
}

impl From<&scrapy_rs_engine::resource_control::ResourceStats> for ResourceStatsDto {
    fn from(stats: &scrapy_rs_engine::resource_control::ResourceStats) -> Self {
        Self {
            memory_usage: stats.memory_usage,
            cpu_usage: stats.cpu_usage,
            active_tasks: stats.active_tasks,
            pending_requests: stats.pending_requests,
        }
    }
}

/// Metrics data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    /// Engine statistics
    pub engine_stats: EngineStats,
    /// Resource statistics
    pub resource_stats: Option<ResourceStatsDto>,
    /// Request rate (requests per second)
    pub request_rate: f64,
    /// Response rate (responses per second)
    pub response_rate: f64,
    /// Item rate (items per second)
    pub item_rate: f64,
    /// Error rate (errors per second)
    pub error_rate: f64,
    /// Request success rate (percentage)
    pub success_rate: f64,
    /// Custom metrics
    pub custom: HashMap<String, f64>,
    /// Timestamp of the last update
    pub last_update: chrono::DateTime<chrono::Utc>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            engine_stats: EngineStats::default(),
            resource_stats: None,
            request_rate: 0.0,
            response_rate: 0.0,
            item_rate: 0.0,
            error_rate: 0.0,
            success_rate: 0.0,
            custom: HashMap::new(),
            last_update: chrono::Utc::now(),
        }
    }
}

/// Metrics collector for the monitoring system
pub struct MetricsCollector {
    /// Current metrics
    metrics: RwLock<Metrics>,
    /// Previous engine stats for rate calculations
    prev_engine_stats: RwLock<EngineStats>,
    /// Last update time for rate calculations
    last_update: RwLock<Instant>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            metrics: RwLock::new(Metrics::default()),
            prev_engine_stats: RwLock::new(EngineStats::default()),
            last_update: RwLock::new(Instant::now()),
        }
    }
    
    /// Update engine statistics
    pub async fn update_engine_stats(&self, stats: &EngineStats) {
        let mut metrics = self.metrics.write().await;
        let mut prev_stats = self.prev_engine_stats.write().await;
        let mut last_update = self.last_update.write().await;
        
        // Calculate rates
        let now = Instant::now();
        let elapsed = now.duration_since(*last_update).as_secs_f64();
        
        if elapsed > 0.0 {
            metrics.request_rate = (stats.request_count - prev_stats.request_count) as f64 / elapsed;
            metrics.response_rate = (stats.response_count - prev_stats.response_count) as f64 / elapsed;
            metrics.item_rate = (stats.item_count - prev_stats.item_count) as f64 / elapsed;
            metrics.error_rate = (stats.error_count - prev_stats.error_count) as f64 / elapsed;
            
            // Calculate success rate
            if stats.request_count > 0 {
                metrics.success_rate = stats.response_count as f64 / stats.request_count as f64 * 100.0;
            }
        }
        
        // Update metrics
        metrics.engine_stats = stats.clone();
        metrics.last_update = chrono::Utc::now();
        
        // Update previous stats and last update time
        *prev_stats = stats.clone();
        *last_update = now;
    }
    
    /// Update resource statistics
    pub async fn update_resource_stats(&self, stats: &scrapy_rs_engine::resource_control::ResourceStats) {
        let mut metrics = self.metrics.write().await;
        metrics.resource_stats = Some(ResourceStatsDto::from(stats));
    }
    
    /// Add a custom metric
    pub async fn add_custom_metric(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        metrics.custom.insert(name.to_string(), value);
    }
    
    /// Get the current metrics
    pub async fn get_metrics(&self) -> Metrics {
        self.metrics.read().await.clone()
    }
} 