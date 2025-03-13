use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::Result;
use scrapy_rs_core::item::DynamicItem;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};
use scrapy_rs_downloader::{DownloaderConfig, HttpDownloader};
use scrapy_rs_engine::{Engine, EngineConfig};
use scrapy_rs_middleware::{
    ChainedRequestMiddleware, ChainedResponseMiddleware, DefaultHeadersMiddleware,
};
use scrapy_rs_pipeline::{JsonFilePipeline, LogPipeline, PipelineType};
use scrapy_rs_scheduler::MemoryScheduler;
use std::collections::HashMap;
use std::sync::Arc;

/// A custom spider that crawls quotes.toscrape.com and extracts quotes
struct QuotesSpider {
    name: String,
    start_urls: Vec<String>,
    allowed_domains: Vec<String>,
}

impl QuotesSpider {
    fn new() -> Self {
        Self {
            name: "quotes".to_string(),
            start_urls: vec!["http://quotes.toscrape.com".to_string()],
            allowed_domains: vec!["quotes.toscrape.com".to_string()],
        }
    }
}

#[async_trait]
impl Spider for QuotesSpider {
    fn name(&self) -> &str {
        &self.name
    }

    fn start_urls(&self) -> Vec<String> {
        self.start_urls.clone()
    }

    fn allowed_domains(&self) -> Vec<String> {
        self.allowed_domains.clone()
    }

    async fn parse(&self, response: Response) -> Result<ParseOutput> {
        // In a real spider, you would use a library like scraper or html5ever to parse the HTML
        // For this example, we'll just create some dummy quotes

        let mut output = ParseOutput::new();

        // Create some dummy quotes
        for i in 1..6 {
            let mut quote = DynamicItem::new("quote");
            quote.set("text", format!("Quote {} from {}", i, response.url));
            quote.set("author", format!("Author {}", i));
            quote.set("tags", serde_json::json!(["tag1", "tag2", "tag3"]));
            output.add_item(quote);
        }

        // Extract the "next" link if it exists
        // In a real spider, you would parse the HTML to find the next link
        if !response.url.to_string().contains("page/2") {
            let next_url = "http://quotes.toscrape.com/page/2/";
            let request = Request::get(next_url)?;
            output.add_request(request);
        }

        Ok(output)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    // Create a spider
    let spider = Arc::new(QuotesSpider::new());

    // Create components
    let scheduler = Arc::new(MemoryScheduler::new());

    let downloader_config = DownloaderConfig {
        concurrent_requests: 2,
        user_agent: "scrapy_rs/0.1.0 (+https://github.com/yourusername/scrapy_rs)".to_string(),
        timeout: 30,
        retry_enabled: true,
        max_retries: 3,
        ..DownloaderConfig::default()
    };
    let downloader = Arc::new(HttpDownloader::new(downloader_config)?);

    // Create pipelines
    let pipelines = vec![
        PipelineType::Log(LogPipeline::info()),
        PipelineType::JsonFile(JsonFilePipeline::new("quotes.json", false)),
    ];
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
        concurrent_requests: 2,
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
