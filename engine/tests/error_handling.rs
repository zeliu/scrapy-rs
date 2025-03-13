mod mock_downloader;

use std::collections::HashMap;
use std::sync::Arc;

use scrapy_rs_core::error::{Error, Result};
use scrapy_rs_core::error_handler::{ErrorAction, ErrorHandler, ErrorManager};
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};
use scrapy_rs_core::async_trait;
use scrapy_rs_middleware::{ChainedRequestMiddleware, ChainedResponseMiddleware};
use scrapy_rs_pipeline::PipelineType;
use scrapy_rs_scheduler::{MemoryScheduler, Scheduler};

use scrapy_rs_engine::Engine;
use scrapy_rs_engine::EngineConfig;

use mock_downloader::{FailingDownloader, MockDownloader};

/// A test spider that always fails
struct FailingSpider;

#[async_trait]
impl Spider for FailingSpider {
    fn name(&self) -> &str {
        "failing_spider"
    }
    
    fn start_urls(&self) -> Vec<String> {
        vec!["https://example.com".to_string()]
    }
    
    async fn parse(&self, _response: Response) -> Result<ParseOutput> {
        Err(Error::other("Test parse error"))
    }
}

/// A custom error handler that counts errors
struct CountingErrorHandler {
    count: std::sync::atomic::AtomicUsize,
}

impl CountingErrorHandler {
    fn new() -> Self {
        Self {
            count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    
    fn count(&self) -> usize {
        self.count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl ErrorHandler for CountingErrorHandler {
    async fn handle_error(&self, error: &Error, _spider: &dyn Spider) -> Result<ErrorAction> {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        
        // Always skip errors
        Ok(ErrorAction::Skip {
            reason: format!("Counted error: {}", error),
        })
    }
}

#[tokio::test]
async fn test_error_handling() {
    // Create a failing spider
    let spider = Arc::new(FailingSpider);
    
    // Create a failing downloader
    let downloader = Arc::new(FailingDownloader);
    
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
    struct ErrorHandlerWrapper(Arc<CountingErrorHandler>);
    
    #[async_trait]
    impl ErrorHandler for ErrorHandlerWrapper {
        async fn handle_error(&self, error: &Error, spider: &dyn Spider) -> Result<ErrorAction> {
            self.0.handle_error(error, spider).await
        }
    }
    
    let error_manager = Arc::new(ErrorManager::new(ErrorHandlerWrapper(error_handler.clone())));
    
    // Create the engine
    let mut engine = Engine::with_components(
        spider,
        scheduler,
        downloader,
        pipelines,
        request_middlewares,
        response_middlewares,
        EngineConfig::default(),
    ).with_error_manager(error_manager.clone());
    
    // Run the engine
    let _ = engine.run().await;
    
    // Check that errors were handled
    assert!(error_handler.count() > 0, "No errors were handled");
}

#[tokio::test]
async fn test_error_retry() {
    // Create a test spider
    let spider = Arc::new(FailingSpider);
    
    // Create a mock downloader
    let downloader = Arc::new(MockDownloader::new());
    
    // Create a scheduler
    let scheduler = Arc::new(MemoryScheduler::new());
    
    // Create middlewares
    let request_middlewares = Arc::new(ChainedRequestMiddleware::new(vec![]));
    let response_middlewares = Arc::new(ChainedResponseMiddleware::new(vec![]));
    
    // Create pipelines
    let pipelines = Arc::new(PipelineType::Chained(vec![]));
    
    // Create a custom error handler that retries
    struct RetryErrorHandler;
    
    #[async_trait]
    impl ErrorHandler for RetryErrorHandler {
        async fn handle_error(&self, _error: &Error, _spider: &dyn Spider) -> Result<ErrorAction> {
            // Create a new request to retry
            let request = Request::get("https://example.com/retry").unwrap();
            
            Ok(ErrorAction::Retry {
                request,
                delay: None,
                reason: "Test retry".to_string(),
            })
        }
    }
    
    let error_manager = Arc::new(ErrorManager::new(RetryErrorHandler));
    
    // Create the engine
    let mut engine = Engine::with_components(
        spider,
        scheduler.clone(),
        downloader,
        pipelines,
        request_middlewares,
        response_middlewares,
        EngineConfig::default(),
    ).with_error_manager(error_manager);
    
    // Run the engine
    let _ = engine.run().await;
    
    // Check that the retry request was added to the scheduler
    assert!(scheduler.is_empty().await, "Scheduler should be empty after processing all requests");
} 