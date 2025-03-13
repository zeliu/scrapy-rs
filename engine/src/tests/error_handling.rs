use std::collections::HashMap;
use std::sync::Arc;

use scrapy_rs_core::error::{Error, ErrorContext, Result};
use scrapy_rs_core::error_handler::{DefaultErrorHandler, ErrorAction, ErrorHandler, ErrorManager};
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};
use scrapy_rs_middleware::{ChainedRequestMiddleware, ChainedResponseMiddleware};
use scrapy_rs_pipeline::DummyPipeline;
use scrapy_rs_scheduler::{MemoryScheduler, Scheduler};

use crate::mock::{FailingDownloader, MockDownloader};
use crate::{Engine, EngineConfig};

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
        Err(Error::Other {
            message: "Test error".to_string(),
            context: ErrorContext::new(),
        })
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

#[scrapy_rs_core::async_trait]
impl ErrorHandler for CountingErrorHandler {
    async fn handle_error(&self, error: &Error, _spider: &dyn Spider) -> Result<ErrorAction> {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        
        // Just skip the error
        Ok(ErrorAction::Skip {
            reason: format!("Skipping error: {}", error),
        })
    }
}

#[tokio::test]
async fn test_error_handling() {
    // Create a test spider
    let spider = Arc::new(FailingSpider);
    
    // Create a custom error handler that counts errors
    let error_handler = Arc::new(CountingErrorHandler::new());
    
    // Create a wrapper that implements ErrorHandler
    struct ErrorHandlerWrapper(Arc<CountingErrorHandler>);
    
    #[scrapy_rs_core::async_trait]
    impl ErrorHandler for ErrorHandlerWrapper {
        async fn handle_error(&self, error: &Error, spider: &dyn Spider) -> Result<ErrorAction> {
            self.0.handle_error(error, spider).await
        }
    }
    
    let error_manager = Arc::new(ErrorManager::new(ErrorHandlerWrapper(error_handler.clone())));
    
    // Create the engine components
    let scheduler = Arc::new(MemoryScheduler::new());
    let downloader = Arc::new(FailingDownloader);
    let dummy_pipeline = DummyPipeline::new();
    let pipelines = Arc::new(dummy_pipeline);
    let request_middlewares = Arc::new(ChainedRequestMiddleware::new(vec![]));
    let response_middlewares = Arc::new(ChainedResponseMiddleware::new(vec![]));
    
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
    
    // Add a test request
    scheduler.enqueue(Request::get("https://example.com").unwrap()).await.unwrap();
    
    // Run the engine
    let result = engine.run().await;
    
    // Engine should complete successfully, but with errors handled
    assert!(result.is_ok(), "Engine should complete successfully");
    
    // Verify that the error handler was called
    assert!(error_handler.count() > 0, "Error handler should have been called");
}

#[tokio::test]
async fn test_error_retry() {
    // Create a test spider
    let spider = Arc::new(FailingSpider);
    
    // Create a test error handler that retries
    struct RetryErrorHandler {
        max_retries: usize,
        retries: std::sync::atomic::AtomicUsize,
    }
    
    impl RetryErrorHandler {
        fn new(max_retries: usize) -> Self {
            Self {
                max_retries,
                retries: std::sync::atomic::AtomicUsize::new(0),
            }
        }
        
        fn retries(&self) -> usize {
            self.retries.load(std::sync::atomic::Ordering::SeqCst)
        }
    }
    
    #[scrapy_rs_core::async_trait]
    impl ErrorHandler for RetryErrorHandler {
        async fn handle_error(&self, _error: &Error, _spider: &dyn Spider) -> Result<ErrorAction> {
            let retries = self.retries.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            
            if retries < self.max_retries {
                // Create a new request to retry
                let request = Request::get("https://example.com/retry").unwrap();
                
                Ok(ErrorAction::Retry {
                    request,
                    delay: None,
                    reason: format!("Test retry {}", retries + 1),
                })
            } else {
                // After max retries, just skip
                Ok(ErrorAction::Skip {
                    reason: "Max retries reached".to_string(),
                })
            }
        }
    }
    
    // Create a wrapper that implements ErrorHandler
    struct ErrorHandlerWrapper(Arc<RetryErrorHandler>);
    
    #[scrapy_rs_core::async_trait]
    impl ErrorHandler for ErrorHandlerWrapper {
        async fn handle_error(&self, error: &Error, spider: &dyn Spider) -> Result<ErrorAction> {
            self.0.handle_error(error, spider).await
        }
    }
    
    let error_handler = Arc::new(RetryErrorHandler::new(3));
    let error_manager = Arc::new(ErrorManager::new(
        ErrorHandlerWrapper(error_handler.clone())
    ));
    
    // Create the engine components
    let scheduler = Arc::new(MemoryScheduler::new());
    let downloader = Arc::new(MockDownloader::new());
    let dummy_pipeline = DummyPipeline::new();
    let pipelines = Arc::new(dummy_pipeline);
    let request_middlewares = Arc::new(ChainedRequestMiddleware::new(vec![]));
    let response_middlewares = Arc::new(ChainedResponseMiddleware::new(vec![]));
    
    // Create the engine with a configuration that limits max retries
    let mut config = EngineConfig::default();
    config.max_retries = 2; // Limit to 2 retries
    
    let mut engine = Engine::with_components(
        spider,
        scheduler.clone(),
        downloader,
        pipelines,
        request_middlewares,
        response_middlewares,
        config,
    ).with_error_manager(error_manager);
    
    // Add a test request
    scheduler.enqueue(Request::get("https://example.com").unwrap()).await.unwrap();
    
    // Run the engine
    let result = engine.run().await;
    
    // Engine should complete successfully after retries
    assert!(result.is_ok(), "Engine should complete successfully");
    
    // Check that retries were attempted
    assert!(error_handler.retries() > 0, "No retries were attempted");
    
    // Scheduler should be empty after processing all requests
    assert!(scheduler.is_empty().await, "Scheduler should be empty after processing all requests");
} 