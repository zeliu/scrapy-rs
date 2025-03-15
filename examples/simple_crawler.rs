use std::collections::HashMap;
use std::time::Duration;

use scrapy_rs_core::error::Result;
use scrapy_rs_core::item::DynamicItem;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};
use scrapy_rs_downloader::{DownloaderConfig, HttpDownloader};
use scrapy_rs_engine::{Engine, EngineConfig, SchedulerType};
use scrapy_rs_middleware::{
    ChainedRequestMiddleware, ChainedResponseMiddleware, DefaultHeadersMiddleware, LogLevel,
    ResponseLoggerMiddleware,
};
use scrapy_rs_pipeline::{JsonFilePipeline, LogPipeline, PipelineType};
use scrapy_rs_scheduler::MemoryScheduler;
use std::sync::Arc;
use tokio::time::sleep;

struct SimpleCrawler {
    name: String,
    start_urls: Vec<String>,
}

impl SimpleCrawler {
    fn new(name: &str, start_urls: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            start_urls,
        }
    }
}

#[async_trait::async_trait]
impl Spider for SimpleCrawler {
    fn name(&self) -> &str {
        &self.name
    }

    fn start_urls(&self) -> Vec<String> {
        self.start_urls.clone()
    }

    async fn parse(&self, response: Response) -> Result<ParseOutput> {
        println!("Parsing: {}", response.url);
        println!("Status: {}", response.status);

        // Create a simple item
        let mut item = DynamicItem::new("page");
        item.set("url", response.url.to_string());
        item.set("status", response.status);
        item.set("title", "Example Page");

        // In a real crawler, you would extract links and create new requests
        // For this example, we'll just return the item
        let mut output = ParseOutput::new();
        output.add_item(item);
        Ok(output)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create a spider
    let spider = Arc::new(SimpleCrawler::new(
        "simple_crawler",
        vec!["https://example.com".to_string()],
    ));

    // Create a scheduler
    let scheduler = Arc::new(MemoryScheduler::new());

    // Create a request middleware chain
    let mut request_middleware = ChainedRequestMiddleware::new(vec![]);
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        "Scrapy-RS/0.1.0 (+https://github.com/yourusername/scrapy-rs)".to_string(),
    );
    request_middleware.add(DefaultHeadersMiddleware::new(headers));

    // Create a response middleware chain
    let response_middleware = ChainedResponseMiddleware::new(vec![Box::new(
        ResponseLoggerMiddleware::new(LogLevel::Info),
    )]);

    // Create pipelines
    let pipelines = vec![
        PipelineType::Log(LogPipeline::info()),
        PipelineType::JsonFile(JsonFilePipeline::new("items.json", false)),
    ];
    let pipeline = Arc::new(PipelineType::Chained(pipelines));

    // Create a downloader
    let downloader_config = DownloaderConfig {
        concurrent_requests: 2,
        user_agent: "Scrapy-RS/0.1.0 (+https://github.com/yourusername/scrapy-rs)".to_string(),
        timeout: 30,
        retry_enabled: true,
        max_retries: 3,
        ..DownloaderConfig::default()
    };
    let downloader = Arc::new(HttpDownloader::new(downloader_config)?);

    // Create an engine
    let engine_config = EngineConfig {
        concurrent_requests: 2,
        scheduler_type: SchedulerType::Memory,
        ..Default::default()
    };

    let mut engine = Engine::with_components(
        spider,
        scheduler,
        downloader,
        pipeline,
        Arc::new(request_middleware),
        Arc::new(response_middleware),
        engine_config,
    );

    // Run the engine
    let stats = engine.run().await?;

    // Print stats
    println!("Crawl completed!");
    println!("Requests: {}", stats.request_count);
    println!("Responses: {}", stats.response_count);
    println!("Items: {}", stats.item_count);
    println!("Errors: {}", stats.error_count);
    if let Some(duration) = stats.duration() {
        println!("Duration: {:?}", duration);
    }
    if let Some(rps) = stats.requests_per_second() {
        println!("Requests per second: {:.2}", rps);
    }

    // Wait a bit to ensure all logs are printed
    sleep(Duration::from_millis(100)).await;

    Ok(())
}
