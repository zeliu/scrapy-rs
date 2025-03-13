use std::sync::Arc;
use std::time::Duration;

use log::{info, LevelFilter};
use scrapy_rs_core::error::Result;
use scrapy_rs_core::item::DynamicItem;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};
use scrapy_rs_engine::{Engine, EngineConfig, SchedulerType};
use scrapy_rs_scheduler::CrawlStrategy;
use env_logger;
use tokio::time::sleep;

struct ExampleSpider {
    name: String,
    start_urls: Vec<String>,
}

impl ExampleSpider {
    fn new(name: &str, start_urls: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            start_urls,
        }
    }
}

#[scrapy_rs_core::async_trait]
impl Spider for ExampleSpider {
    fn name(&self) -> &str {
        &self.name
    }

    fn start_urls(&self) -> Vec<String> {
        self.start_urls.clone()
    }

    async fn parse(&self, response: Response) -> Result<ParseOutput> {
        let mut output = ParseOutput::new();
        
        // Extract a simple item
        let mut item = DynamicItem::new("example_item");
        item.set("url", response.url.to_string());
        item.set("title", "Example Domain");
        
        output.add_item(item);
        
        // Add some follow-up requests with different depths
        if response.url.path() == "/" {
            for i in 1..=5 {
                let mut url = response.url.clone();
                url.set_path(&format!("/page{}", i));
                
                if let Ok(mut request) = Request::get(url.to_string()) {
                    // Set depth for BFS/DFS scheduling
                    request.meta.insert("depth".to_string(), serde_json::json!(i));
                    
                    // Set different priorities for priority-based scheduling
                    request = request.with_priority(5 - i);
                    
                    output.add_request(request);
                }
            }
        }
        
        Ok(output)
    }
}

async fn run_with_scheduler(scheduler_type: SchedulerType, crawl_strategy: CrawlStrategy) -> Result<()> {
    // Create a spider
    let spider = Arc::new(ExampleSpider::new(
        "example_spider",
        vec!["https://example.com/".to_string()],
    ));
    
    // Create engine configuration
    let config = EngineConfig {
        scheduler_type,
        crawl_strategy,
        concurrent_requests: 2,
        download_delay_ms: 100,
        max_depth: Some(3),
        domain_delay_ms: Some(500),
        max_requests_per_domain: Some(10),
        ..EngineConfig::default()
    };
    
    // Create and run the engine
    let mut engine = Engine::new(spider)?;
    engine = engine.with_config(config);
    
    info!("Running crawler with scheduler: {:?}, strategy: {:?}", scheduler_type, crawl_strategy);
    let stats = engine.run().await?;
    
    info!("Crawler finished with stats: {:?}", stats);
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();
    
    // Run with different scheduler types
    info!("=== Running with Memory Scheduler ===");
    run_with_scheduler(SchedulerType::Memory, CrawlStrategy::Priority).await?;
    sleep(Duration::from_secs(1)).await;
    
    info!("=== Running with Domain Group Scheduler (Priority) ===");
    run_with_scheduler(SchedulerType::DomainGroup, CrawlStrategy::Priority).await?;
    sleep(Duration::from_secs(1)).await;
    
    info!("=== Running with Domain Group Scheduler (BFS) ===");
    run_with_scheduler(SchedulerType::DomainGroup, CrawlStrategy::BreadthFirst).await?;
    sleep(Duration::from_secs(1)).await;
    
    info!("=== Running with Domain Group Scheduler (DFS) ===");
    run_with_scheduler(SchedulerType::DomainGroup, CrawlStrategy::DepthFirst).await?;
    sleep(Duration::from_secs(1)).await;
    
    info!("=== Running with BreadthFirst Scheduler ===");
    run_with_scheduler(SchedulerType::BreadthFirst, CrawlStrategy::BreadthFirst).await?;
    sleep(Duration::from_secs(1)).await;
    
    info!("=== Running with DepthFirst Scheduler ===");
    run_with_scheduler(SchedulerType::DepthFirst, CrawlStrategy::DepthFirst).await?;
    
    Ok(())
} 