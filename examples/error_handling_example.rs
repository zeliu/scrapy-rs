use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use scrapy_rs_core::error::{Error, Result};
use scrapy_rs_core::error_handler::{ErrorAction, ErrorHandler, ErrorManager};
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};
use scrapy_rs_downloader::HttpDownloader;
use scrapy_rs_middleware::{ChainedRequestMiddleware, ChainedResponseMiddleware};
use scrapy_rs_pipeline::PipelineType;
use scrapy_rs_scheduler::MemoryScheduler;

use scrapy_rs_engine::Engine;
use scrapy_rs_engine::EngineConfig;

/// A test spider that always fails
struct FailingSpider;

#[scrapy_rs_core::async_trait]
impl Spider for FailingSpider {
    fn name(&self) -> &str {
        "failing_spider"
    }

    fn start_urls(&self) -> Vec<String> {
        vec!["https://example.com".to_string()]
    }

    async fn parse(&self, _response: Response) -> Result<ParseOutput> {
        Err(Box::new(Error::other("Test error in parse method")))
    }
}

/// A custom error handler that counts errors
struct CountingErrorHandler {
    count: AtomicUsize,
}

impl CountingErrorHandler {
    fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
        }
    }

    fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

#[scrapy_rs_core::async_trait]
impl ErrorHandler for CountingErrorHandler {
    async fn handle_error(&self, error: &Error, _spider: &dyn Spider) -> Result<ErrorAction> {
        self.count.fetch_add(1, Ordering::SeqCst);
        println!("Error handled: {}", error);

        // Just skip the error
        Ok(ErrorAction::Skip {
            reason: format!("Skipping error: {}", error),
        })
    }
}

/// Wrapper for Arc<CountingErrorHandler>
struct ErrorHandlerWrapper(Arc<CountingErrorHandler>);

#[scrapy_rs_core::async_trait]
impl ErrorHandler for ErrorHandlerWrapper {
    async fn handle_error(&self, error: &Error, spider: &dyn Spider) -> Result<ErrorAction> {
        self.0.handle_error(error, spider).await
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    // Create a test spider
    let spider = Arc::new(FailingSpider);

    // Create a scheduler
    let scheduler = Arc::new(MemoryScheduler::new());

    // Create middlewares
    let request_middlewares = Arc::new(ChainedRequestMiddleware::new(vec![]));
    let response_middlewares = Arc::new(ChainedResponseMiddleware::new(vec![]));

    // Create pipelines
    let pipelines = Arc::new(PipelineType::Chained(vec![]));

    // Create a custom error handler
    let error_handler = Arc::new(CountingErrorHandler::new());

    // Create a wrapper that implements ErrorHandler
    let error_manager = Arc::new(ErrorManager::new(ErrorHandlerWrapper(
        error_handler.clone(),
    )));

    // Create the engine
    let mut engine = Engine::with_components(
        spider,
        scheduler,
        Arc::new(HttpDownloader::default()),
        pipelines,
        request_middlewares,
        response_middlewares,
        EngineConfig::default(),
    )
    .with_error_manager(error_manager);

    // Run the engine
    let stats = engine.run().await?;

    // Print stats
    println!("Crawl completed!");
    println!("Requests: {}", stats.request_count);
    println!("Responses: {}", stats.response_count);
    println!("Items: {}", stats.item_count);
    println!("Errors: {}", stats.error_count);
    println!("Duration: {:?}", stats.duration().unwrap());

    // Verify that the error handler was called
    println!("Error handler was called {} times", error_handler.count());

    Ok(())
}
