use std::time::{Duration, Instant};

use crate::slot::{create_request_id, Slot, SlotManager};
use scrapy_rs_core::request::Request;

/// Test basic slot functionality
#[tokio::test]
async fn test_slot_basic_operations() {
    let slot = Slot::new("test-domain.com".to_string(), 5000, 0);

    // Check initial state
    assert_eq!(slot.name(), "test-domain.com");
    assert!(slot.is_idle().await);
    assert_eq!(slot.active_count().await, 0);
    assert_eq!(slot.queue_length().await, 0);

    // Create some test requests
    let request1 = Request::get("https://test-domain.com/path1").unwrap();

    let request2 = Request::get("https://test-domain.com/path2").unwrap();

    // Add requests to the slot
    let _rx1 = slot.add_request(request1.clone()).await;
    let _rx2 = slot.add_request(request2.clone()).await;

    // Check queue state
    assert_eq!(slot.queue_length().await, 2);
    assert!(!slot.is_idle().await);

    // Get a request from the queue
    let next = slot.next_request().await;
    assert!(next.is_some());

    // Check updated state
    assert_eq!(slot.queue_length().await, 1);
    assert_eq!(slot.active_count().await, 1);

    // Finish the request
    let (request, _) = next.unwrap();
    let request_id = create_request_id(&request);
    slot.finish_request(&request_id, Some(1500)).await;

    // Check state after finishing
    assert_eq!(slot.active_count().await, 0);
    assert_eq!(slot.queue_length().await, 1);
    assert!(!slot.is_idle().await);

    // Get the second request
    let next = slot.next_request().await;
    assert!(next.is_some());

    // Check final queue state
    assert_eq!(slot.queue_length().await, 0);
    assert_eq!(slot.active_count().await, 1);

    // Finish the second request
    let (request, _) = next.unwrap();
    let request_id = create_request_id(&request);
    slot.finish_request(&request_id, Some(2000)).await;

    // Check final state
    assert!(slot.is_idle().await);
    assert_eq!(slot.active_count().await, 0);
    assert_eq!(slot.queue_length().await, 0);
}

/// Test slot delay functionality
#[tokio::test]
async fn test_slot_delay() {
    // Create a slot with a 100ms delay
    let slot = Slot::new("test-domain.com".to_string(), 5000, 100);

    // Create some test requests
    let request1 = Request::get("https://test-domain.com/path1").unwrap();

    let request2 = Request::get("https://test-domain.com/path2").unwrap();

    // Add requests to the slot
    let _rx1 = slot.add_request(request1.clone()).await;
    let _rx2 = slot.add_request(request2.clone()).await;

    // Get the first request (should be immediate)
    let start_time = Instant::now();
    let next1 = slot.next_request().await;
    let elapsed1 = start_time.elapsed();
    assert!(next1.is_some());
    assert!(elapsed1 < Duration::from_millis(50)); // Should be quick

    // Get the second request (should be delayed)
    let start_time = Instant::now();
    let next2 = slot.next_request().await;
    let elapsed2 = start_time.elapsed();
    assert!(next2.is_some());
    assert!(elapsed2 >= Duration::from_millis(100)); // Should be delayed
}

/// Test slot manager functionality
#[tokio::test]
async fn test_slot_manager() {
    // Create a slot manager
    let manager = SlotManager::new(5000, 0);

    // Get slots for different domains
    let slot1 = manager.get_slot("domain1.com").await;
    let slot2 = manager.get_slot("domain2.com").await;

    // Check that we got different slots
    assert_eq!(slot1.name(), "domain1.com");
    assert_eq!(slot2.name(), "domain2.com");

    // Add requests to different slots
    let request1 = Request::get("https://domain1.com/path").unwrap();

    let request2 = Request::get("https://domain2.com/path").unwrap();

    let _rx1 = slot1.add_request(request1.clone()).await;
    let _rx2 = slot2.add_request(request2.clone()).await;

    // Check queue states
    assert_eq!(slot1.queue_length().await, 1);
    assert_eq!(slot2.queue_length().await, 1);

    // Get slot names
    let names = manager.get_slot_names().await;
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"domain1.com".to_string()));
    assert!(names.contains(&"domain2.com".to_string()));

    // Get a slot by name
    let slot1_again = manager.get_slot_by_name("domain1.com").await;
    assert!(slot1_again.is_some());
    assert_eq!(slot1_again.unwrap().name(), "domain1.com");

    // Check if all slots are idle
    assert!(!manager.all_slots_idle().await);

    // Process all requests
    let next1 = slot1.next_request().await.unwrap();
    let next2 = slot2.next_request().await.unwrap();

    let request1_id = create_request_id(&next1.0);
    let request2_id = create_request_id(&next2.0);
    slot1.finish_request(&request1_id, None).await;
    slot2.finish_request(&request2_id, None).await;

    // Now all slots should be idle
    assert!(manager.all_slots_idle().await);

    // Close a slot
    manager.close_slot("domain1.com").await;

    // Add request to closed slot
    let request3 = Request::get("https://domain1.com/another-path").unwrap();

    let _rx3 = slot1.add_request(request3.clone()).await;

    // Try to get a request from the slot (should return None because slot is closing)
    let next = slot1.next_request().await;
    assert!(next.is_none());
}
