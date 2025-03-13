use std::sync::Arc;
use std::time::Duration;

use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};
use scrapy_rs_core::item::DynamicItem;
use scrapy_rs_middleware::{ChainedRequestMiddleware, ChainedResponseMiddleware};
use scrapy_rs_pipeline::{LogPipeline, PipelineType};
use scrapy_rs_scheduler::MemoryScheduler;
use scrapy_rs_downloader::HttpDownloader;

use scrapy_rs_engine::{Engine, EngineConfig};
use scrapy_rs_engine::resource_control::ResourceLimits;

/// A memory-intensive spider that allocates large vectors
struct MemoryIntensiveSpider {
    name: String,
    start_urls: Vec<String>,
    memory_blocks: Vec<Vec<u8>>,
}

impl MemoryIntensiveSpider {
    fn new(name: &str, start_urls: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            start_urls,
            memory_blocks: Vec::new(),
        }
    }
    
    fn allocate_memory(&mut self, size_mb: usize) {
        // Allocate a block of memory (size in MB)
        let block = vec![0u8; size_mb * 1024 * 1024];
        self.memory_blocks.push(block);
        println!("Allocated {}MB of memory, total blocks: {}", size_mb, self.memory_blocks.len());
    }
}

#[scrapy_rs_core::async_trait]
impl Spider for MemoryIntensiveSpider {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn start_urls(&self) -> Vec<String> {
        self.start_urls.clone()
    }
    
    async fn parse(&self, response: Response) -> Result<ParseOutput> {
        // Simulate memory-intensive processing
        let mut output = ParseOutput::default();
        
        // Extract some links
        let html = response.text()?;
        let links = html.matches("href=\"").count();
        println!("Found {} links on {}", links, response.url);
        
        // Add a new request for each link (limit to 5 to avoid too many requests)
        for i in 0..std::cmp::min(5, links) {
            let url = format!("https://example.com/page/{}", i);
            if let Ok(request) = Request::get(&url) {
                output.requests.push(request);
            }
        }
        
        // Add an item
        let mut item = DynamicItem::new("page");
        item.set("url", response.url.to_string())
           .set("title", format!("Page {}", response.url))
           .set("links", links);
        output.items.push(item);
        
        Ok(output)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Create a spider
    let spider = Arc::new(MemoryIntensiveSpider::new(
        "memory_intensive",
        vec!["https://example.com".to_string()],
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
    
    // Configure resource limits
    let resource_limits = ResourceLimits {
        max_memory: 100 * 1024 * 1024, // 100 MB
        max_cpu: 50.0,                 // 50% CPU
        max_tasks: 5,                  // Max 5 concurrent tasks
        max_pending_requests: 10,      // Max 10 pending requests
        throttle_factor: 0.5,          // Throttle by 50%
        monitor_interval_ms: 1000,     // Check every second
    };
    
    // Create engine config
    let mut config = EngineConfig::default();
    config.resource_limits = resource_limits;
    config.enable_resource_monitoring = true;
    config.concurrent_requests = 2;
    
    // Create the engine
    let mut engine = Engine::with_components(
        spider.clone(),
        scheduler,
        downloader,
        pipelines,
        request_middlewares,
        response_middlewares,
        config,
    );
    
    // Allocate some memory in the spider to trigger resource limits
    if let Some(spider_mut) = Arc::get_mut(&mut spider.clone()) {
        // Allocate 20MB chunks of memory every second
        for i in 1..6 {
            spider_mut.allocate_memory(20);
            println!("Allocated {}MB total", i * 20);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    
    // Run the engine
    let stats = engine.run().await?;
    
    // Print stats
    println!("Crawl completed!");
    println!("Requests: {}", stats.request_count);
    println!("Responses: {}", stats.response_count);
    println!("Items: {}", stats.item_count);
    println!("Errors: {}", stats.error_count);
    println!("Duration: {:?}", stats.duration().unwrap());
    
    Ok(())
} 