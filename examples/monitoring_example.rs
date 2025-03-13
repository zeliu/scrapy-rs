use std::sync::Arc;
use std::time::Duration;

use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{BasicSpider, ParseOutput, Spider};
use scrapy_rs_middleware::{ChainedRequestMiddleware, ChainedResponseMiddleware};
use scrapy_rs_pipeline::{LogPipeline, PipelineType};
use scrapy_rs_scheduler::MemoryScheduler;
use scrapy_rs_downloader::HttpDownloader;
use scrapy_rs_engine::{Engine, EngineConfig};
use scrapy_rs_monitoring::{Monitoring, MonitoringConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Create a spider
    let spider = Arc::new(BasicSpider::new(
        "monitoring_example",
        vec!["https://quotes.toscrape.com".to_string()],
    ));
    
    // Create a scheduler
    let scheduler = Arc::new(MemoryScheduler::new());
    
    // Create a downloader
    let downloader = Arc::new(HttpDownloader::default()?);
    
    // Create middlewares
    let request_middlewares = Arc::new(ChainedRequestMiddleware::new(vec![]));
    let response_middlewares = Arc::new(ChainedResponseMiddleware::new(vec![]));
    
    // Create pipelines
    let pipelines = Arc::new(PipelineType::Log(LogPipeline::info()));
    
    // Create engine config
    let mut config = EngineConfig::default();
    config.concurrent_requests = 2;
    
    // Create the engine
    let engine = Arc::new(Engine::with_components(
        spider.clone(),
        scheduler,
        downloader,
        pipelines,
        request_middlewares,
        response_middlewares,
        config,
    ));
    
    // Create monitoring config
    let monitoring_config = MonitoringConfig {
        enabled: true,
        metrics_interval_ms: 1000,
        max_events: 1000,
        enable_dashboard: true,
        dashboard_port: 8080,
        enable_metrics_server: true,
        metrics_server_port: 9090,
    };
    
    // Create and attach monitoring
    let mut monitoring = Monitoring::new(monitoring_config);
    monitoring.attach(engine.clone()).await?;
    
    println!("Monitoring started!");
    println!("Dashboard: http://localhost:8080");
    println!("Metrics: http://localhost:9090/metrics");
    
    // Run the engine
    let mut engine_mut = engine.clone();
    let engine_handle = tokio::spawn(async move {
        engine_mut.run().await
    });
    
    // Wait for the engine to complete or timeout after 60 seconds
    tokio::select! {
        result = engine_handle => {
            match result {
                Ok(Ok(stats)) => {
                    println!("Crawl completed!");
                    println!("Requests: {}", stats.request_count);
                    println!("Responses: {}", stats.response_count);
                    println!("Items: {}", stats.item_count);
                    println!("Errors: {}", stats.error_count);
                    println!("Duration: {:?}", stats.duration().unwrap());
                }
                Ok(Err(e)) => {
                    println!("Engine error: {}", e);
                }
                Err(e) => {
                    println!("Task error: {}", e);
                }
            }
        }
        _ = tokio::time::sleep(Duration::from_secs(60)) => {
            println!("Timeout reached, stopping engine...");
        }
    }
    
    // Stop monitoring
    monitoring.stop().await?;
    
    Ok(())
} 