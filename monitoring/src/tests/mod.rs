use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::json;
use tokio::sync::RwLock;
use tokio::time::sleep;

use scrapy_rs_core::error::Result;
use scrapy_rs_engine::EngineStats;
use scrapy_rs_engine::resource_control::ResourceStats;

use crate::metrics::{MetricsCollector, ResourceStatsDto};
use crate::{Monitoring, MonitoringConfig, MonitoringEvent};

#[tokio::test]
async fn test_metrics_collector() -> Result<()> {
    // Create a metrics collector
    let collector = MetricsCollector::new();
    
    // Create some engine stats
    let mut stats = EngineStats::default();
    stats.requests_sent = 10;
    stats.responses_received = 8;
    stats.items_scraped = 5;
    stats.errors = 2;
    
    // Update the metrics
    collector.update_engine_stats(&stats).await;
    
    // Create some resource stats
    let resource_stats = ResourceStats {
        memory_usage: 1024 * 1024, // 1MB
        cpu_usage: 5.0,            // 5%
        active_tasks: 3,
        pending_requests: 5,
    };
    
    // Update the resource stats
    collector.update_resource_stats(&resource_stats).await;
    
    // Add a custom metric
    collector.add_custom_metric("test_metric", 42.0).await;
    
    // Get the metrics
    let metrics = collector.get_metrics().await;
    
    // Check the metrics
    assert_eq!(metrics.engine_stats.requests_sent, 10);
    assert_eq!(metrics.engine_stats.responses_received, 8);
    assert_eq!(metrics.engine_stats.items_scraped, 5);
    assert_eq!(metrics.engine_stats.errors, 2);
    
    // Check resource stats
    let resource_stats_dto = metrics.resource_stats.unwrap();
    assert_eq!(resource_stats_dto.memory_usage, 1024 * 1024);
    assert_eq!(resource_stats_dto.cpu_usage, 5.0);
    assert_eq!(resource_stats_dto.active_tasks, 3);
    assert_eq!(resource_stats_dto.pending_requests, 5);
    
    // Check custom metrics
    assert_eq!(metrics.custom.get("test_metric"), Some(&42.0));
    
    Ok(())
}

#[tokio::test]
async fn test_monitoring_events() -> Result<()> {
    // Create a monitoring config
    let config = MonitoringConfig {
        enabled: true,
        metrics_interval_ms: 100,
        max_events: 5,
        enable_dashboard: false,
        dashboard_port: 8080,
        enable_metrics_server: false,
        metrics_server_port: 8081,
    };
    
    // Create a monitoring instance
    let monitoring = Monitoring::new(config);
    
    // Send some events
    monitoring.event_tx.send(MonitoringEvent::EngineStarted).unwrap();
    monitoring.event_tx.send(MonitoringEvent::SpiderOpened { name: "test_spider".to_string() }).unwrap();
    monitoring.event_tx.send(MonitoringEvent::RequestScheduled { url: "https://example.com".to_string() }).unwrap();
    
    // Wait for events to be processed
    sleep(Duration::from_millis(50)).await;
    
    // Get the events
    let events = monitoring.get_events().await;
    
    // Check the events
    assert_eq!(events.len(), 3);
    
    // Check event types
    match &events[0].1 {
        MonitoringEvent::EngineStarted => (),
        _ => panic!("Expected EngineStarted event"),
    }
    
    match &events[1].1 {
        MonitoringEvent::SpiderOpened { name } => {
            assert_eq!(name, "test_spider");
        },
        _ => panic!("Expected SpiderOpened event"),
    }
    
    match &events[2].1 {
        MonitoringEvent::RequestScheduled { url } => {
            assert_eq!(url, "https://example.com");
        },
        _ => panic!("Expected RequestScheduled event"),
    }
    
    // Test max events limit
    monitoring.event_tx.send(MonitoringEvent::ResponseReceived { url: "https://example.com".to_string(), status: 200 }).unwrap();
    monitoring.event_tx.send(MonitoringEvent::ItemScraped { item_type: "test_item".to_string() }).unwrap();
    monitoring.event_tx.send(MonitoringEvent::EngineStopped).unwrap(); // This should push out the oldest event
    
    // Wait for events to be processed
    sleep(Duration::from_millis(50)).await;
    
    // Get the events again
    let events = monitoring.get_events().await;
    
    // Check the events (should be limited to 5)
    assert_eq!(events.len(), 5);
    
    // The first event should now be SpiderOpened (EngineStarted was pushed out)
    match &events[0].1 {
        MonitoringEvent::SpiderOpened { name } => {
            assert_eq!(name, "test_spider");
        },
        _ => panic!("Expected SpiderOpened event"),
    }
    
    // Test custom events
    monitoring.send_event("custom_event", json!({ "value": 42 })).unwrap();
    
    // Wait for events to be processed
    sleep(Duration::from_millis(50)).await;
    
    // Get the events again
    let events = monitoring.get_events().await;
    
    // Check the custom event
    let last_event = &events[events.len() - 1].1;
    match last_event {
        MonitoringEvent::Custom { name, data } => {
            assert_eq!(name, "custom_event");
            assert_eq!(data["value"], 42);
        },
        _ => panic!("Expected Custom event"),
    }
    
    Ok(())
}

#[tokio::test]
async fn test_resource_stats_dto() {
    // Create a ResourceStats
    let resource_stats = ResourceStats {
        memory_usage: 2048 * 1024, // 2MB
        cpu_usage: 10.5,           // 10.5%
        active_tasks: 7,
        pending_requests: 12,
    };
    
    // Convert to ResourceStatsDto
    let dto = ResourceStatsDto::from(&resource_stats);
    
    // Check the conversion
    assert_eq!(dto.memory_usage, 2048 * 1024);
    assert_eq!(dto.cpu_usage, 10.5);
    assert_eq!(dto.active_tasks, 7);
    assert_eq!(dto.pending_requests, 12);
    
    // Test serialization
    let json = serde_json::to_string(&dto).unwrap();
    let deserialized: ResourceStatsDto = serde_json::from_str(&json).unwrap();
    
    // Check deserialized values
    assert_eq!(deserialized.memory_usage, 2048 * 1024);
    assert_eq!(deserialized.cpu_usage, 10.5);
    assert_eq!(deserialized.active_tasks, 7);
    assert_eq!(deserialized.pending_requests, 12);
} 