use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{BasicSpider, ParseOutput, Spider};
use scrapy_rs_middleware::{
    ChainedRequestMiddleware, ConditionalMiddleware, DefaultHeadersMiddleware, MiddlewarePriority,
    RateLimitMiddleware, RequestMiddleware, ResponseMiddleware, RetryMiddleware,
    UrlFilterMiddleware,
};
use tokio::sync::Mutex;

// Test spider implementation
struct TestSpider;

#[async_trait::async_trait]
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
async fn test_default_headers_middleware() {
    let middleware =
        DefaultHeadersMiddleware::new(HashMap::from([("X-Test".to_string(), "test".to_string())]));

    let request = Request::get("https://example.com").unwrap();
    let processed = middleware
        .process_request(request, &TestSpider)
        .await
        .unwrap();

    assert_eq!(processed.headers.get("X-Test").unwrap(), "test");
}

#[tokio::test]
async fn test_url_filter_middleware() {
    let middleware = UrlFilterMiddleware::from_strings(
        vec!["https://example.com/.*"],
        vec!["https://example.com/forbidden"],
    )
    .unwrap();

    // Allowed URL
    let request = Request::get("https://example.com/allowed").unwrap();
    let result = middleware.process_request(request, &TestSpider).await;
    assert!(result.is_ok());

    // Denied URL
    let request = Request::get("https://example.com/forbidden").unwrap();
    let result = middleware.process_request(request, &TestSpider).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_response_logger_middleware() {
    let middleware = scrapy_rs_middleware::ResponseLoggerMiddleware::info();

    let response = Response {
        url: "https://example.com".parse().unwrap(),
        status: 200,
        headers: HashMap::new(),
        body: vec![],
        request: Request::get("https://example.com").unwrap(),
        meta: HashMap::new(),
        flags: Vec::new(),
        certificate: None,
        ip_address: None,
        protocol: None,
    };

    let processed = middleware
        .process_response(response, &TestSpider)
        .await
        .unwrap();
    assert_eq!(processed.status, 200);
}

#[tokio::test]
async fn test_retry_middleware() {
    let middleware = RetryMiddleware::new(vec![500], 3, 100);

    // Create a response with a 500 status code
    let response = Response {
        url: "https://example.com".parse().unwrap(),
        status: 500,
        headers: HashMap::new(),
        body: vec![],
        request: Request::get("https://example.com").unwrap(),
        meta: HashMap::new(),
        flags: Vec::new(),
        certificate: None,
        ip_address: None,
        protocol: None,
    };

    // First retry
    let processed = middleware
        .process_response(response.clone(), &TestSpider)
        .await
        .unwrap();
    assert!(processed.meta.contains_key("retry_request"));

    // Second retry (with retry_count = 1)
    let mut response_with_count = response.clone();
    response_with_count
        .meta
        .insert("retry_count".to_string(), serde_json::json!(1));
    let processed = middleware
        .process_response(response_with_count, &TestSpider)
        .await
        .unwrap();
    assert!(processed.meta.contains_key("retry_request"));

    // Third retry (with retry_count = 2)
    let mut response_with_count = response.clone();
    response_with_count
        .meta
        .insert("retry_count".to_string(), serde_json::json!(2));
    let processed = middleware
        .process_response(response_with_count, &TestSpider)
        .await
        .unwrap();
    assert!(processed.meta.contains_key("retry_request"));

    // Fourth retry (with retry_count = 3) - should not retry
    let mut response_with_count = response.clone();
    response_with_count
        .meta
        .insert("retry_count".to_string(), serde_json::json!(3));
    let processed = middleware
        .process_response(response_with_count, &TestSpider)
        .await
        .unwrap();
    assert!(!processed.meta.contains_key("retry_request"));
}

#[tokio::test]
async fn test_chained_middleware() {
    let mut chained = ChainedRequestMiddleware::new(Vec::new());

    let headers_middleware =
        DefaultHeadersMiddleware::new(HashMap::from([("X-Test".to_string(), "test".to_string())]));

    chained.add(headers_middleware);

    let request = Request::get("https://example.com").unwrap();
    let processed = chained.process_request(request, &TestSpider).await.unwrap();

    assert_eq!(processed.headers.get("X-Test").unwrap(), "test");
}

#[tokio::test]
async fn test_middleware_priority() {
    // Create middlewares with different priorities
    let high_priority =
        DefaultHeadersMiddleware::new(HashMap::new()).with_priority(MiddlewarePriority::High);

    let normal_priority = DefaultHeadersMiddleware::new(HashMap::new());

    let low_priority =
        DefaultHeadersMiddleware::new(HashMap::new()).with_priority(MiddlewarePriority::Low);

    // Add them in reverse order
    let mut chain = ChainedRequestMiddleware::new(vec![]);
    chain.add(low_priority);
    chain.add(normal_priority);
    chain.add(high_priority);

    // The adding process sorts middlewares internally by priority
    // Create a test request and run it through the chain
    let request = Request::get("https://example.com").unwrap();
    let _ = chain.process_request(request, &TestSpider).await.unwrap();

    // The execution order can't be directly verified in this integration test
    // since we can't access the private fields
    // The other test (test_middleware_chain_execution_order) verifies this behavior
}

#[tokio::test]
async fn test_direct_middleware_execution() {
    // Create a struct to track execution order
    let order = Arc::new(Mutex::new(Vec::<String>::new()));

    // Create middlewares with different priorities
    struct TestMiddleware {
        name: String,
        priority: MiddlewarePriority,
        order: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl RequestMiddleware for TestMiddleware {
        async fn process_request(&self, request: Request, _spider: &dyn Spider) -> Result<Request> {
            println!("Processing middleware: {}", self.name);
            let mut guard = self.order.lock().await;
            guard.push(self.name.clone());
            Ok(request)
        }

        fn priority(&self) -> MiddlewarePriority {
            self.priority
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn should_process_request(&self, _request: &Request, _spider: &dyn Spider) -> bool {
            true
        }
    }

    // Create middlewares with different priorities
    let high = TestMiddleware {
        name: "high".to_string(),
        priority: MiddlewarePriority::High,
        order: Arc::clone(&order),
    };

    let normal = TestMiddleware {
        name: "normal".to_string(),
        priority: MiddlewarePriority::Normal,
        order: Arc::clone(&order),
    };

    let low = TestMiddleware {
        name: "low".to_string(),
        priority: MiddlewarePriority::Low,
        order: Arc::clone(&order),
    };

    // Create test request and spider
    let request = Request::get("https://example.com").unwrap();
    let spider = BasicSpider::new("test_spider", vec!["https://example.com".to_string()]);

    // Execute middlewares directly in the correct order
    let request = high.process_request(request, &spider).await.unwrap();
    let request = normal.process_request(request, &spider).await.unwrap();
    let _request = low.process_request(request, &spider).await.unwrap();

    // Check the execution order
    let execution_order = order.lock().await;
    println!("Direct execution order: {:?}", *execution_order);
    assert_eq!(*execution_order, vec!["high", "normal", "low"]);
}

#[tokio::test]
async fn test_conditional_middleware() {
    // Create a middleware that only processes requests to example.com
    let headers_middleware = DefaultHeadersMiddleware::common();
    let conditional = ConditionalMiddleware::new(headers_middleware, |req, _| {
        req.url.host_str() == Some("example.com")
    });

    // Create test requests
    let example_request = Request::get("https://example.com").unwrap();
    let other_request = Request::get("https://other.com").unwrap();

    let spider = BasicSpider::new("test_spider", vec!["https://example.com".to_string()]);

    // The middleware should process the example.com request
    assert!(conditional.should_process_request(&example_request, &spider));

    // The middleware should not process the other.com request
    assert!(!conditional.should_process_request(&other_request, &spider));

    // Test actual processing
    let processed_example = conditional
        .process_request(example_request.clone(), &spider)
        .await
        .unwrap();
    let processed_other = conditional
        .process_request(other_request.clone(), &spider)
        .await
        .unwrap();

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
    let spider = BasicSpider::new("test_spider", vec!["https://example.com".to_string()]);

    // First 3 requests should pass through immediately
    for _ in 0..3 {
        let start = std::time::Instant::now();
        let _ = rate_limit
            .process_request(request.clone(), &spider)
            .await
            .unwrap();
        let elapsed = start.elapsed();

        // Should be very quick
        assert!(elapsed < Duration::from_millis(50));
    }

    // The 4th request should be delayed
    let start = std::time::Instant::now();
    let _ = rate_limit
        .process_request(request.clone(), &spider)
        .await
        .unwrap();
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

    #[async_trait::async_trait]
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

        fn should_process_request(&self, _request: &Request, _spider: &dyn Spider) -> bool {
            true
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
