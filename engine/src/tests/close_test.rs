use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::Engine;
use scrapy_rs_core::error::Result;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};

struct TestSpider {
    name: String,
    start_urls: Vec<String>,
    closed_flag: Arc<AtomicBool>,
    // For testing timeout scenarios
    hang_on_close: bool,
}

#[scrapy_rs_core::async_trait]
impl Spider for TestSpider {
    fn name(&self) -> &str {
        &self.name
    }

    fn start_urls(&self) -> Vec<String> {
        self.start_urls.clone()
    }

    async fn parse(&self, _response: Response) -> Result<ParseOutput> {
        Ok(ParseOutput::new())
    }

    async fn closed(&self) -> Result<()> {
        if self.hang_on_close {
            // Simulate a scenario where the close operation hangs
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        self.closed_flag.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn test_engine_close() {
    // Create a test spider with a close flag
    let closed_flag = Arc::new(AtomicBool::new(false));
    let spider = Arc::new(TestSpider {
        name: "test_close_spider".to_string(),
        start_urls: vec!["http://example.com".to_string()],
        closed_flag: closed_flag.clone(),
        hang_on_close: false,
    });

    // Create engine
    let engine = Engine::new(spider).unwrap();

    // Call close method
    let close_result = engine.close().await;
    assert!(
        close_result.is_ok(),
        "Close operation failed: {:?}",
        close_result
    );

    // Verify that the spider's closed method was called
    assert!(
        closed_flag.load(Ordering::SeqCst),
        "Spider's closed method was not called"
    );
}

#[tokio::test]
async fn test_engine_close_with_timeout() {
    // Create a spider that hangs during close
    let closed_flag = Arc::new(AtomicBool::new(false));
    let spider = Arc::new(TestSpider {
        name: "test_timeout_spider".to_string(),
        start_urls: vec!["http://example.com".to_string()],
        closed_flag: closed_flag.clone(),
        hang_on_close: true,
    });

    // Create engine
    let engine = Engine::new(spider).unwrap();

    // Call close method - should timeout but not hang
    let start = std::time::Instant::now();
    let close_result = engine.close().await;
    let elapsed = start.elapsed();

    // Verify results
    assert!(
        close_result.is_ok(),
        "Close operation failed: {:?}",
        close_result
    );

    // Verify time - should be less than 10 seconds (due to our 5-second timeout) but more than 4 seconds (to ensure timeout mechanism works)
    assert!(
        elapsed < Duration::from_secs(10),
        "Close took too long: {:?}",
        elapsed
    );
    assert!(
        elapsed > Duration::from_secs(4),
        "Close did not wait for timeout: {:?}",
        elapsed
    );

    // Spider's closed method should not be marked as closed due to timeout
    assert!(
        !closed_flag.load(Ordering::SeqCst),
        "Spider's closed flag should not be set due to timeout"
    );
}
