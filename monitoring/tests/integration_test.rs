use std::sync::Arc;
use std::time::Duration;

use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::Spider;
use scrapy_rs_core::types::ParseOutput;
use scrapy_rs_downloader::reqwest::ReqwestDownloader;
use scrapy_rs_engine::{Engine, EngineBuilder};
use scrapy_rs_middleware::DefaultHeadersMiddleware;
use scrapy_rs_monitoring::{Monitoring, MonitoringConfig, MonitoringEvent};
use scrapy_rs_pipeline::ItemCollectorPipeline;
use scrapy_rs_scheduler::memory::MemoryScheduler;

use tokio::time::sleep;

// Simple test spider
struct TestSpider;

impl Spider for TestSpider {
    fn name(&self) -> &str {
        "test_spider"
    }

    fn start_urls(&self) -> Vec<String> {
        vec!["https://httpbin.org/get".to_string()]
    }

    async fn parse(&self, response: Response) -> Result<ParseOutput> {
        let mut output = ParseOutput::default();
        output.add_item(response.url().to_string());
        Ok(output)
    }
}

#[tokio::test]
async fn test_monitoring_integration() -> Result<()> {
    // Create a monitoring config
    let config = MonitoringConfig {
        enabled: true,
        metrics_interval_ms: 100,
        max_events: 100,
        enable_dashboard: false,
        dashboard_port: 8080,
        enable_metrics_server: false,
        metrics_server_port: 8081,
    };
    
    // Create a monitoring instance
    let mut monitoring = Monitoring::new(config);
    
    // Create a spider
    let spider = Arc::new(TestSpider);
    
    // Create a scheduler
    let scheduler = Arc::new(MemoryScheduler::new());
    
    // Create a downloader
    let downloader = Arc::new(ReqwestDownloader::new());
    
    // Create middlewares
    let middlewares = DefaultHeadersMiddleware::common();
    
    // Create pipelines
    let pipelines = Arc::new(ItemCollectorPipeline::new());
    
    // Create an engine
    let engine = EngineBuilder::new()
        .spider(spider)
        .scheduler(scheduler)
        .downloader(downloader)
        .request_middlewares(middlewares)
        .item_pipelines(pipelines.clone())
        .build();
    
    // Attach monitoring to the engine
    monitoring.attach(Arc::new(engine)).await?;
    
    // Start the engine in a separate task
    let engine_handle = tokio::spawn(async move {
        // Wait a bit to ensure monitoring is ready
        sleep(Duration::from_millis(100)).await;
        
        // Start the engine
        let _ = engine.start().await;
    });
    
    // Wait for the engine to complete (with a timeout)
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            // Check if we have events
            let events = monitoring.get_events().await;
            
            // If we've received EngineStopped, we're done
            if events.iter().any(|(_, e)| matches!(e, MonitoringEvent::EngineStopped)) {
                break;
            }
            
            // Wait a bit before checking again
            sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("Test timed out");
    
    // Get all events
    let events = monitoring.get_events().await;
    
    // Check that we received the expected events
    assert!(events.iter().any(|(_, e)| matches!(e, MonitoringEvent::EngineStarted)));
    assert!(events.iter().any(|(_, e)| matches!(e, MonitoringEvent::SpiderOpened { .. })));
    assert!(events.iter().any(|(_, e)| matches!(e, MonitoringEvent::RequestScheduled { .. })));
    assert!(events.iter().any(|(_, e)| matches!(e, MonitoringEvent::RequestSent { .. })));
    assert!(events.iter().any(|(_, e)| matches!(e, MonitoringEvent::ResponseReceived { .. })));
    assert!(events.iter().any(|(_, e)| matches!(e, MonitoringEvent::ItemScraped { .. })));
    assert!(events.iter().any(|(_, e)| matches!(e, MonitoringEvent::StatsUpdated { .. })));
    assert!(events.iter().any(|(_, e)| matches!(e, MonitoringEvent::EngineStopped)));
    
    // Check metrics
    let metrics = monitoring.get_metrics().await;
    assert!(metrics.engine_stats.requests_sent > 0);
    assert!(metrics.engine_stats.responses_received > 0);
    assert!(metrics.engine_stats.items_scraped > 0);
    
    // Check collected items
    let items = pipelines.get_items();
    assert!(!items.is_empty());
    
    // Stop monitoring
    monitoring.stop().await?;
    
    // Wait for engine to finish
    let _ = engine_handle.await;
    
    Ok(())
} 