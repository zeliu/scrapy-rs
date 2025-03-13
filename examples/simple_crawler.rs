use scrapy_rs_core::item::DynamicItem;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};
use scrapy_rs_engine::{Engine, EngineConfig};
use scrapy_rs_middleware::{DefaultHeadersMiddleware, RequestMiddleware, ChainedRequestMiddleware, ChainedResponseMiddleware, ResponseMiddleware};
use scrapy_rs_pipeline::{JsonFilePipeline, Pipeline, PipelineType, LogPipeline};
use scrapy_rs_scheduler::MemoryScheduler;
use scrapy_rs_downloader::{HttpDownloader, DownloaderConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::init();

    // Create a basic spider
    let spider = Arc::new(BasicSpider::new(
        "example",
        vec!["https://example.com".to_string()],
    ));

    // Create components
    let scheduler = Arc::new(MemoryScheduler::new());
    
    let downloader_config = DownloaderConfig {
        concurrent_requests: 5,
        user_agent: "scrapy_rs/0.1.0 (+https://github.com/yourusername/scrapy_rs)".to_string(),
        timeout: 30,
        retry_enabled: true,
        max_retries: 3,
        ..DownloaderConfig::default()
    };
    let downloader = Arc::new(HttpDownloader::new(downloader_config)?);
    
    // Create pipelines
    let mut pipelines = Vec::new();
    pipelines.push(PipelineType::Log(LogPipeline::info()));
    pipelines.push(PipelineType::JsonFile(JsonFilePipeline::new("items.json", false)));
    let pipeline = Arc::new(PipelineType::Chained(pipelines));
    
    // Create request middlewares
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        "RS-Spider/0.1.0 (+https://github.com/yourusername/rs-spider)".to_string(),
    );
    let mut request_middlewares = ChainedRequestMiddleware::new(vec![]);
    request_middlewares.add(DefaultHeadersMiddleware::new(headers));
    
    // Create response middlewares
    let response_middlewares = ChainedResponseMiddleware::new(vec![]);
    
    // Configure the engine
    let config = EngineConfig {
        concurrent_requests: 5,
        download_delay_ms: 1000,
        respect_robots_txt: true,
        user_agent: "RS-Spider/0.1.0 (+https://github.com/yourusername/rs-spider)".to_string(),
        log_requests: true,
        log_items: true,
        log_stats: true,
        ..EngineConfig::default()
    };
    
    // Create the engine with all components
    let mut engine = Engine::with_components(
        spider,
        scheduler,
        downloader,
        pipeline,
        Arc::new(request_middlewares),
        Arc::new(response_middlewares),
        config,
    );

    // Run the engine
    let stats = engine.run().await?;

    // Print the results
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

    Ok(())
}

/// A basic spider implementation
struct BasicSpider {
    name: String,
    start_urls: Vec<String>,
}

impl BasicSpider {
    fn new(name: &str, start_urls: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            start_urls,
        }
    }
}

#[scrapy_rs_core::async_trait]
impl Spider for BasicSpider {
    fn name(&self) -> &str {
        &self.name
    }

    fn start_urls(&self) -> Vec<String> {
        self.start_urls.clone()
    }

    async fn parse(&self, response: Response) -> scrapy_rs_core::error::Result<ParseOutput> {
        let mut output = ParseOutput::new();
        
        // Create a simple item with the page title and URL
        let mut item = DynamicItem::new("page");
        item.set("url", response.url.to_string());
        item.set("status", response.status as u64);
        
        // Extract a simple title from the HTML
        let body_str = String::from_utf8_lossy(&response.body);
        if let Some(title_start) = body_str.find("<title>") {
            if let Some(title_end) = body_str.find("</title>") {
                let title = &body_str[title_start + 7..title_end];
                item.set("title", title.to_string());
            }
        }
        
        output.add_item(item);
        
        Ok(output)
    }
} 