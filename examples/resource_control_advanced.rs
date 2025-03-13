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
use scrapy_rs_engine::resource_control::{ResourceLimits, ResourceStats};

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
        
        // Simulate CPU-intensive work
        let mut sum: u64 = 0;
        for i in 0..1_000_000 {
            sum = sum.wrapping_add(i);
        }
        println!("Completed CPU-intensive work: sum = {}", sum);
        
        // Add a new request for each link (limit to 10 to avoid too many requests)
        for i in 0..std::cmp::min(10, links) {
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
        
        // Simulate some processing delay
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        Ok(output)
    }
}

async fn run_with_resource_control() -> Result<()> {
    println!("=== Running with resource control enabled ===");
    
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
    
    // Configure resource limits - set strict limits
    let resource_limits = ResourceLimits {
        max_memory: 50 * 1024 * 1024, // 50 MB
        max_cpu: 50.0,                // 50% CPU
        max_tasks: 2,                 // Max 2 concurrent tasks
        max_pending_requests: 5,      // Max 5 pending requests
        throttle_factor: 0.5,         // Throttle by 50%
        monitor_interval_ms: 500,     // Check every 500ms
    };
    
    // Create engine config
    let mut config = EngineConfig::default();
    config.resource_limits = resource_limits;
    config.enable_resource_monitoring = true;
    config.concurrent_requests = 5; // Try to use 5 concurrent requests, but resource control will limit to 2
    
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
        // Allocate 10MB chunks of memory every second
        for i in 1..6 {
            spider_mut.allocate_memory(10);
            println!("Allocated {}MB total", i * 10);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    
    // Create a separate task to monitor resource stats
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    let stats_task = tokio::spawn(async move {
        while let Some(stats) = rx.recv().await {
            print_resource_stats(&stats);
        }
    });
    
    // Start a background task to periodically check resource stats
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let monitor_handle = tokio::spawn(async move {
        for _ in 0..30 {
            interval.tick().await;
            // This task will exit after 30 seconds
        }
    });
    
    // Run the engine and monitor resources
    let start = std::time::Instant::now();
    
    // Create a task to run the engine
    let engine_handle = tokio::spawn(async move {
        engine.run().await
    });
    
    // Monitor resources while the engine is running
    while !engine_handle.is_finished() {
        // Check resource stats every second
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Get resource stats from a new engine instance
        let config = EngineConfig::default();
        let temp_engine = Engine::new(spider.clone())?;
        if let Some(stats) = temp_engine.get_resource_stats().await {
            let _ = tx.send(stats).await;
        }
    }
    
    // Wait for the engine to complete
    let stats = engine_handle.await.unwrap()?;
    let duration = start.elapsed();
    
    // Cancel the monitoring tasks
    monitor_handle.abort();
    drop(tx);
    let _ = stats_task.await;
    
    // Print stats
    println!("Crawl completed with resource control!");
    println!("Requests: {}", stats.request_count);
    println!("Responses: {}", stats.response_count);
    println!("Items: {}", stats.item_count);
    println!("Errors: {}", stats.error_count);
    println!("Duration: {:?}", duration);
    
    Ok(())
}

async fn run_without_resource_control() -> Result<()> {
    println!("\n=== Running without resource control ===");
    
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
    
    // Create engine config without resource control
    let mut config = EngineConfig::default();
    config.enable_resource_monitoring = false;
    config.concurrent_requests = 5; // Use 5 concurrent requests
    
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
    
    // Allocate some memory in the spider
    if let Some(spider_mut) = Arc::get_mut(&mut spider.clone()) {
        // Allocate 10MB chunks of memory every second
        for i in 1..6 {
            spider_mut.allocate_memory(10);
            println!("Allocated {}MB total", i * 10);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    
    // Run the engine
    let start = std::time::Instant::now();
    let stats = engine.run().await?;
    let duration = start.elapsed();
    
    // Print stats
    println!("Crawl completed without resource control!");
    println!("Requests: {}", stats.request_count);
    println!("Responses: {}", stats.response_count);
    println!("Items: {}", stats.item_count);
    println!("Errors: {}", stats.error_count);
    println!("Duration: {:?}", duration);
    
    Ok(())
}

fn print_resource_stats(stats: &ResourceStats) {
    println!("Resource Stats:");
    println!("  Memory: {:.2} MB", stats.memory_usage as f64 / 1024.0 / 1024.0);
    println!("  CPU: {:.2}%", stats.cpu_usage);
    println!("  Active Tasks: {}", stats.active_tasks);
    println!("  Pending Requests: {}", stats.pending_requests);
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Run with resource control
    run_with_resource_control().await?;
    
    // Run without resource control
    run_without_resource_control().await?;
    
    Ok(())
} 