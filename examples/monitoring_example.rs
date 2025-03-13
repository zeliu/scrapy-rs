use std::sync::Arc;

use scrapy_rs_core::error::Result;
use scrapy_rs_core::spider::BasicSpider;
use scrapy_rs_downloader::HttpDownloader;
use scrapy_rs_engine::{Engine, EngineConfig};
use scrapy_rs_middleware::{ChainedRequestMiddleware, ChainedResponseMiddleware};
use scrapy_rs_pipeline::{LogPipeline, PipelineType};
use scrapy_rs_scheduler::MemoryScheduler;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    // Create a spider
    let spider = Arc::new(BasicSpider::new(
        "example_spider",
        vec!["https://example.com".to_string()],
    ));

    // Create a scheduler
    let scheduler = Arc::new(MemoryScheduler::new());

    // Create a downloader
    let downloader = Arc::new(HttpDownloader::default());

    // Create middlewares
    let request_middlewares = Arc::new(ChainedRequestMiddleware::new(vec![]));
    let response_middlewares = Arc::new(ChainedResponseMiddleware::new(vec![]));

    // Create pipelines
    let pipelines = Arc::new(PipelineType::Log(LogPipeline::info()));

    // Create engine configuration
    let config = EngineConfig {
        concurrent_requests: 5,
        ..Default::default()
    };

    // Create engine
    let mut engine = Engine::with_components(
        spider,
        scheduler,
        downloader,
        pipelines,
        request_middlewares,
        response_middlewares,
        config,
    );

    // Run the engine
    let stats = engine.run().await?;

    // Print statistics
    println!("Crawl completed!");
    println!("Requests: {}", stats.request_count);
    println!("Responses: {}", stats.response_count);
    println!("Items: {}", stats.item_count);
    println!("Errors: {}", stats.error_count);

    if let Some(duration) = stats.duration() {
        println!("Duration: {:.2} seconds", duration.as_secs_f64());
    }

    if let Some(rps) = stats.requests_per_second() {
        println!("Requests per second: {:.2}", rps);
    }

    Ok(())
}
