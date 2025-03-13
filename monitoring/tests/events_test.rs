use std::sync::Arc;
use std::time::Duration;

use scrapy_rs_core::error::Result;
use scrapy_rs_engine::EngineStats;
use scrapy_rs_engine::resource_control::ResourceStats;
use scrapy_rs_monitoring::{Monitoring, MonitoringConfig, MonitoringEvent};
use serde_json::json;
use tokio::time::sleep;

#[tokio::test]
async fn test_event_system() -> Result<()> {
    // Create a monitoring config
    let config = MonitoringConfig {
        enabled: true,
        metrics_interval_ms: 100,
        max_events: 10,
        enable_dashboard: false,
        dashboard_port: 8080,
        enable_metrics_server: false,
        metrics_server_port: 8081,
    };
    
    // Create a monitoring instance
    let monitoring = Monitoring::new(config);
    
    // Send various events
    monitoring.send_event("test_event", json!({ "value": 42 })).unwrap();
    
    // Wait for events to be processed
    sleep(Duration::from_millis(50)).await;
    
    // Get the events
    let events = monitoring.get_events().await;
    
    // Check that we received the custom event
    assert!(events.iter().any(|(_, e)| {
        if let MonitoringEvent::Custom { name, data } = e {
            name == "test_event" && data["value"] == 42
        } else {
            false
        }
    }));
    
    // Test event limit
    for i in 0..15 {
        monitoring.send_event(&format!("event_{}", i), json!({ "index": i })).unwrap();
    }
    
    // Wait for events to be processed
    sleep(Duration::from_millis(50)).await;
    
    // Get the events again
    let events = monitoring.get_events().await;
    
    // Check that we have at most 10 events (the limit)
    assert!(events.len() <= 10);
    
    // The oldest events should have been removed
    assert!(events.iter().all(|(_, e)| {
        if let MonitoringEvent::Custom { name, .. } = e {
            !name.starts_with("event_0") && !name.starts_with("event_1") && !name.starts_with("event_2") && !name.starts_with("event_3") && !name.starts_with("event_4")
        } else {
            true
        }
    }));
    
    Ok(())
}

#[tokio::test]
async fn test_metrics_update() -> Result<()> {
    // Create a monitoring config
    let config = MonitoringConfig {
        enabled: true,
        metrics_interval_ms: 100,
        max_events: 10,
        enable_dashboard: false,
        dashboard_port: 8080,
        enable_metrics_server: false,
        metrics_server_port: 8081,
    };
    
    // Create a monitoring instance
    let mut monitoring = Monitoring::new(config);
    
    // Create a mock engine with stats
    let mut engine_stats = EngineStats::default();
    engine_stats.requests_sent = 100;
    engine_stats.responses_received = 90;
    engine_stats.items_scraped = 80;
    engine_stats.errors = 10;
    
    // Create a mock engine
    let engine = MockEngine::new(engine_stats.clone());
    
    // Attach the engine to monitoring
    monitoring.attach(Arc::new(engine)).await?;
    
    // Wait for metrics to be collected
    sleep(Duration::from_millis(200)).await;
    
    // Get the metrics
    let metrics = monitoring.get_metrics().await;
    
    // Check engine stats
    assert_eq!(metrics.engine_stats.requests_sent, 100);
    assert_eq!(metrics.engine_stats.responses_received, 90);
    assert_eq!(metrics.engine_stats.items_scraped, 80);
    assert_eq!(metrics.engine_stats.errors, 10);
    
    // Stop monitoring
    monitoring.stop().await?;
    
    Ok(())
}

// Mock engine for testing
struct MockEngine {
    stats: EngineStats,
}

impl MockEngine {
    fn new(stats: EngineStats) -> Self {
        Self { stats }
    }
}

impl scrapy_rs_engine::Engine {
    // Mock implementation for testing
    pub fn get_stats(&self) -> EngineStats {
        EngineStats::default()
    }
} 