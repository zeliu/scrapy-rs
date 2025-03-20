use super::*;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::serde_json;

#[tokio::test]
async fn test_memory_scheduler() {
    let scheduler = MemoryScheduler::new();

    // Create some test requests with different priorities
    let req1 = Request::get("https://example.com/1")
        .unwrap()
        .with_priority(1);
    let req2 = Request::get("https://example.com/2")
        .unwrap()
        .with_priority(2);
    let req3 = Request::get("https://example.com/3")
        .unwrap()
        .with_priority(3);

    // Enqueue the requests
    scheduler.enqueue(req1.clone()).await.unwrap();
    scheduler.enqueue(req2.clone()).await.unwrap();
    scheduler.enqueue(req3.clone()).await.unwrap();

    // Check the length
    assert_eq!(scheduler.len().await, 3);

    // Check that we've seen these URLs
    assert!(scheduler.has_seen("https://example.com/1").await);
    assert!(scheduler.has_seen("https://example.com/2").await);
    assert!(scheduler.has_seen("https://example.com/3").await);

    // Try to enqueue a duplicate request
    scheduler.enqueue(req1.clone()).await.unwrap();

    // Length should still be 3
    assert_eq!(scheduler.len().await, 3);

    // Get the next requests in priority order (highest first)
    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/3");

    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/2");

    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/1");

    // Queue should now be empty
    assert!(scheduler.is_empty().await);
    assert!(scheduler.next().await.is_none());

    // Clear the scheduler
    scheduler.clear().await.unwrap();

    // URLs should no longer be seen
    assert!(!scheduler.has_seen("https://example.com/1").await);
}

#[tokio::test]
async fn test_fifo_scheduler() {
    let scheduler = FifoScheduler::new();

    // Create some test requests
    let req1 = Request::get("https://example.com/1").unwrap();
    let req2 = Request::get("https://example.com/2").unwrap();
    let req3 = Request::get("https://example.com/3").unwrap();

    // Enqueue the requests
    scheduler.enqueue(req1.clone()).await.unwrap();
    scheduler.enqueue(req2.clone()).await.unwrap();
    scheduler.enqueue(req3.clone()).await.unwrap();

    // Check the length
    assert_eq!(scheduler.len().await, 3);

    // Get the next requests in FIFO order
    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/1");

    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/2");

    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/3");

    // Queue should now be empty
    assert!(scheduler.is_empty().await);
}

#[tokio::test]
async fn test_domain_group_scheduler() {
    let config = SchedulerConfig {
        strategy: CrawlStrategy::Priority,
        max_requests_per_domain: Some(2),
        domain_delay_ms: None,
        domain_whitelist: None,
        domain_blacklist: None,
        respect_depth: true,
        max_depth: None,
    };

    let scheduler = DomainGroupScheduler::new(config);

    // Create requests for different domains with different priorities
    let req1 = Request::get("https://example.com/1")
        .unwrap()
        .with_priority(1);
    let req2 = Request::get("https://example.org/1")
        .unwrap()
        .with_priority(2);
    let req3 = Request::get("https://example.net/1")
        .unwrap()
        .with_priority(3);
    let req4 = Request::get("https://example.com/2")
        .unwrap()
        .with_priority(4);

    // Enqueue the requests
    scheduler.enqueue(req1).await.unwrap();
    scheduler.enqueue(req2).await.unwrap();
    scheduler.enqueue(req3).await.unwrap();
    scheduler.enqueue(req4).await.unwrap();

    // Check the length
    assert_eq!(scheduler.len().await, 4);

    // Get the next requests in priority order (highest first)
    // Should get example.com/2 first (priority 4)
    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/2");

    // Then example.net/1 (priority 3)
    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.net/1");

    // Then example.org/1 (priority 2)
    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.org/1");

    // Then example.com/1 (priority 1)
    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/1");

    // Queue should now be empty
    assert!(scheduler.is_empty().await);
}

#[tokio::test]
async fn test_depth_first_scheduler() {
    let scheduler = DepthFirstScheduler::new(Some(2));

    // Create requests with different depths
    let mut req1 = Request::get("https://example.com/1").unwrap();
    req1.meta.insert("depth".to_string(), serde_json::json!(0));

    let mut req2 = Request::get("https://example.com/2").unwrap();
    req2.meta.insert("depth".to_string(), serde_json::json!(1));

    let mut req3 = Request::get("https://example.com/3").unwrap();
    req3.meta.insert("depth".to_string(), serde_json::json!(2));

    let mut req4 = Request::get("https://example.com/4").unwrap();
    req4.meta.insert("depth".to_string(), serde_json::json!(3)); // Exceeds max depth

    // Enqueue the requests
    scheduler.enqueue(req1.clone()).await.unwrap();
    scheduler.enqueue(req2.clone()).await.unwrap();
    scheduler.enqueue(req3.clone()).await.unwrap();
    scheduler.enqueue(req4.clone()).await.unwrap();

    // Check the length - req4 should be skipped due to depth limit
    assert_eq!(scheduler.len().await, 3);

    // Get the next requests in LIFO order
    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/3");

    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/2");

    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/1");

    // Queue should now be empty
    assert!(scheduler.is_empty().await);
}

#[tokio::test]
async fn test_breadth_first_scheduler() {
    let scheduler = BreadthFirstScheduler::new(Some(2));

    // Create requests with different depths
    let mut req1 = Request::get("https://example.com/1").unwrap();
    req1.meta.insert("depth".to_string(), serde_json::json!(0));

    let mut req2 = Request::get("https://example.com/2").unwrap();
    req2.meta.insert("depth".to_string(), serde_json::json!(1));

    let mut req3 = Request::get("https://example.com/3").unwrap();
    req3.meta.insert("depth".to_string(), serde_json::json!(2));

    let mut req4 = Request::get("https://example.com/4").unwrap();
    req4.meta.insert("depth".to_string(), serde_json::json!(3)); // Exceeds max depth

    // Enqueue the requests in order
    scheduler.enqueue(req1.clone()).await.unwrap();
    scheduler.enqueue(req2.clone()).await.unwrap();
    scheduler.enqueue(req3.clone()).await.unwrap();
    scheduler.enqueue(req4.clone()).await.unwrap();

    // Check the length - req4 should be skipped due to depth limit
    assert_eq!(scheduler.len().await, 3);

    // Get the next requests in FIFO order
    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/1");

    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/2");

    let next = scheduler.next().await.unwrap();
    assert_eq!(next.url.as_str(), "https://example.com/3");

    // Queue should now be empty
    assert!(scheduler.is_empty().await);
}
