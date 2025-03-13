use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};

use crate::{
    ChainedRequestMiddleware, ConditionalMiddleware,
    DefaultHeadersMiddleware, MiddlewarePriority, RateLimitMiddleware,
    RequestMiddleware,
};

// Test spider implementation
struct TestSpider;

impl Spider for TestSpider {
    fn name(&self) -> &str {
        "test_spider"
    }

    fn start_urls(&self) -> Vec<String> {
        vec!["https://example.com".to_string()]
    }

    async fn parse(&self, _response: Response) -> Result<ParseOutput> {
        Ok(ParseOutput::default())
    }
}

#[tokio::test]
async fn test_middleware_priority() {
    // Create middlewares with different priorities
    let high_priority = DefaultHeadersMiddleware::new(HashMap::new())
        .with_priority(MiddlewarePriority::High);
    
    let normal_priority = DefaultHeadersMiddleware::new(HashMap::new());
    
    let low_priority = DefaultHeadersMiddleware::new(HashMap::new())
        .with_priority(MiddlewarePriority::Low);
    
    // Add them in reverse order
    let mut chain = ChainedRequestMiddleware::new(vec![]);
    chain.add(low_priority);
    chain.add(normal_priority);
    chain.add(high_priority);
    
    // Get middlewares - they should be sorted by priority (highest first)
    let middlewares = chain.get_middlewares();
    
    assert_eq!(middlewares[0].priority, MiddlewarePriority::High);
    assert_eq!(middlewares[1].priority, MiddlewarePriority::Normal);
    assert_eq!(middlewares[2].priority, MiddlewarePriority::Low);
}

#[tokio::test]
async fn test_conditional_middleware() {
    // Create a middleware that only processes requests to example.com
    let headers_middleware = DefaultHeadersMiddleware::common();
    let conditional = ConditionalMiddleware::new(
        headers_middleware,
        |req, _| req.url.host_str() == Some("example.com")
    );
    
    // Create test requests
    let example_request = Request::get("https://example.com").unwrap();
    let other_request = Request::get("https://other.com").unwrap();
    
    let spider = TestSpider;
    
    // The middleware should process the example.com request
    assert!(conditional.should_process_request(&example_request, &spider));
    
    // The middleware should not process the other.com request
    assert!(!conditional.should_process_request(&other_request, &spider));
    
    // Test actual processing
    let processed_example = conditional.process_request(example_request.clone(), &spider).await.unwrap();
    let processed_other = conditional.process_request(other_request.clone(), &spider).await.unwrap();
    
    // The example.com request should have been modified (headers added)
    assert!(processed_example.headers.len() > example_request.headers.len());
    
    // The other.com request should remain unchanged
    assert_eq!(processed_other.headers.len(), other_request.headers.len());
}

#[tokio::test]
async fn test_rate_limit_middleware() {
    // Create a rate limit middleware that allows 3 requests per second
    let rate_limit = RateLimitMiddleware::new(3, 1);
    
    let request = Request::get("https://example.com").unwrap();
    let spider = TestSpider;
    
    // First 3 requests should pass through immediately
    for _ in 0..3 {
        let start = std::time::Instant::now();
        let _ = rate_limit.process_request(request.clone(), &spider).await.unwrap();
        let elapsed = start.elapsed();
        
        // Should be very quick
        assert!(elapsed < Duration::from_millis(50));
    }
    
    // The 4th request should be delayed
    let start = std::time::Instant::now();
    let _ = rate_limit.process_request(request.clone(), &spider).await.unwrap();
    let elapsed = start.elapsed();
    
    // Should be delayed close to 1 second (the time window)
    // We use a slightly lower threshold to account for timing variations
    assert!(elapsed > Duration::from_millis(900));
}

#[tokio::test]
async fn test_middleware_chain_execution_order() {
    // Create a struct to track execution order
    struct OrderTracker {
        order: Arc<tokio::sync::Mutex<Vec<String>>>,
    }
    
    struct TrackingMiddleware {
        name: String,
        tracker: Arc<OrderTracker>,
        priority: MiddlewarePriority,
    }
    
    impl RequestMiddleware for TrackingMiddleware {
        async fn process_request(&self, request: Request, _spider: &dyn Spider) -> Result<Request> {
            let mut order = self.tracker.order.lock().await;
            order.push(self.name.clone());
            Ok(request)
        }
        
        fn priority(&self) -> MiddlewarePriority {
            self.priority
        }
        
        fn name(&self) -> &str {
            &self.name
        }
    }
    
    // Create tracker and middlewares
    let tracker = Arc::new(OrderTracker {
        order: Arc::new(tokio::sync::Mutex::new(Vec::new())),
    });
    
    let high = TrackingMiddleware {
        name: "high".to_string(),
        tracker: tracker.clone(),
        priority: MiddlewarePriority::High,
    };
    
    let normal = TrackingMiddleware {
        name: "normal".to_string(),
        tracker: tracker.clone(),
        priority: MiddlewarePriority::Normal,
    };
    
    let low = TrackingMiddleware {
        name: "low".to_string(),
        tracker: tracker.clone(),
        priority: MiddlewarePriority::Low,
    };
    
    // Create chain and add middlewares in random order
    let mut chain = ChainedRequestMiddleware::new(vec![]);
    chain.add(normal);
    chain.add(low);
    chain.add(high);
    
    // Process a request
    let request = Request::get("https://example.com").unwrap();
    let spider = TestSpider;
    
    let _ = chain.process_request(request, &spider).await.unwrap();
    
    // Check execution order
    let order = tracker.order.lock().await;
    assert_eq!(*order, vec!["high", "normal", "low"]);
} 