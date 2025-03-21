use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{debug, info};
use tokio::sync::{oneshot, RwLock};
use tokio::time::sleep;

use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_scheduler::Scheduler;

/// Minimum response size (in bytes) for calculating active size
const MIN_RESPONSE_SIZE: usize = 1024;

/// The result of processing a request
pub type ProcessResult = oneshot::Sender<Result<Response>>;

/// A tuple of (Request, ProcessResult)
pub type QueueTuple = (Request, ProcessResult);

/// Create a unique identifier for a request based on its URL and method
pub fn create_request_id(request: &Request) -> String {
    format!("{}-{:?}", request.url.as_str(), request.method)
}

/// Downloader slot (one per domain)
pub struct Slot {
    /// The name of the slot (usually the domain)
    name: String,

    /// Maximum size of all active requests (in bytes)
    max_active_size: usize,

    /// Queue of pending requests
    queue: Arc<RwLock<VecDeque<QueueTuple>>>,

    /// Set of active requests
    active: Arc<RwLock<HashMap<String, Request>>>,

    /// Current active size (in bytes)
    active_size: Arc<RwLock<usize>>,

    /// The time when the last request was processed
    last_request_time: Arc<RwLock<Option<Instant>>>,

    /// Delay between requests (in milliseconds)
    delay: u64,

    /// Whether the slot is closing
    is_closing: Arc<RwLock<bool>>,

    /// Scheduler reference if needed
    #[allow(dead_code)]
    scheduler: Option<Arc<dyn Scheduler>>,
}

impl Slot {
    /// Create a new slot
    pub fn new(name: String, max_active_size: usize, delay: u64) -> Self {
        Self {
            name,
            max_active_size,
            queue: Arc::new(RwLock::new(VecDeque::new())),
            active: Arc::new(RwLock::new(HashMap::new())),
            active_size: Arc::new(RwLock::new(0)),
            last_request_time: Arc::new(RwLock::new(None)),
            delay,
            is_closing: Arc::new(RwLock::new(false)),
            scheduler: None,
        }
    }

    /// Create a new slot with a scheduler
    pub fn with_scheduler(
        name: String,
        max_active_size: usize,
        delay: u64,
        scheduler: Arc<dyn Scheduler>,
    ) -> Self {
        Self {
            name,
            max_active_size,
            queue: Arc::new(RwLock::new(VecDeque::new())),
            active: Arc::new(RwLock::new(HashMap::new())),
            active_size: Arc::new(RwLock::new(0)),
            last_request_time: Arc::new(RwLock::new(None)),
            delay,
            is_closing: Arc::new(RwLock::new(false)),
            scheduler: Some(scheduler),
        }
    }

    /// Get the name of the slot
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add a request to the queue
    pub async fn add_request(&self, request: Request) -> oneshot::Receiver<Result<Response>> {
        let (tx, rx) = oneshot::channel();

        // If we have a scheduler, add to it first
        if let Some(scheduler) = &self.scheduler {
            if let Err(e) = scheduler.enqueue(request.clone()).await {
                debug!("Failed to enqueue request in scheduler: {}", e);
            }
        }

        // Also add to our queue
        {
            let mut queue = self.queue.write().await;
            queue.push_back((request, tx));
        }

        rx
    }

    /// Get the next request from the queue
    pub async fn next_request(&self) -> Option<QueueTuple> {
        // Check if the slot is closing
        if *self.is_closing.read().await {
            return None;
        }

        // Check if we need to wait based on delay
        if let Some(last_time) = *self.last_request_time.read().await {
            let elapsed = last_time.elapsed();
            let delay_duration = Duration::from_millis(self.delay);

            if elapsed < delay_duration {
                // We need to wait before processing the next request
                let wait_time = delay_duration - elapsed;
                sleep(wait_time).await;
            }
        }

        // Try to get the next request from the scheduler or the queue
        let request_tuple = {
            let mut queue = self.queue.write().await;

            if queue.is_empty() {
                None
            } else if let Some(scheduler) = &self.scheduler {
                if let Some(next_request) = scheduler.next().await {
                    // Find the matching request in our queue
                    if let Some(pos) = queue.iter().position(|(req, _)| {
                        req.url == next_request.url && req.method == next_request.method
                    }) {
                        // Found it, remove from queue
                        queue.remove(pos)
                    } else {
                        // Not found (unusual case), just get the next one
                        queue.pop_front()
                    }
                } else {
                    // No request from scheduler, use queue
                    queue.pop_front()
                }
            } else {
                // No scheduler, just use queue directly
                queue.pop_front()
            }
        };

        if let Some((request, result_sender)) = request_tuple {
            // Update the last request time
            *self.last_request_time.write().await = Some(Instant::now());

            // Add to active requests
            {
                let mut active = self.active.write().await;
                // Create a unique request identifier
                let request_id = create_request_id(&request);
                active.insert(request_id, request.clone());
            }

            // Update the active size
            {
                let mut active_size = self.active_size.write().await;
                *active_size += MIN_RESPONSE_SIZE; // Initial size, will be updated after response
            }

            Some((request, result_sender))
        } else {
            None
        }
    }

    /// Finish processing a request
    pub async fn finish_request(&self, request_id: &str, response_size: Option<usize>) {
        // Update the active size
        let final_size = response_size.unwrap_or(MIN_RESPONSE_SIZE);
        let effective_size = std::cmp::max(final_size, MIN_RESPONSE_SIZE);

        {
            let mut active_size = self.active_size.write().await;
            // First remove the initial minimum size
            *active_size = active_size.saturating_sub(MIN_RESPONSE_SIZE);
            // Then add the actual response size
            *active_size += effective_size;
        }

        // Remove from active requests
        {
            let mut active = self.active.write().await;
            active.remove(request_id);
        }

        // Check if we can close the slot
        self.check_if_closing().await;
    }

    /// Check if the slot is idle
    pub async fn is_idle(&self) -> bool {
        let queue_empty = self.queue.read().await.is_empty();
        let active_empty = self.active.read().await.is_empty();

        queue_empty && active_empty
    }

    /// Check if the slot needs to throttle
    pub async fn needs_throttle(&self) -> bool {
        let active_size = *self.active_size.read().await;
        active_size > self.max_active_size
    }

    /// Start closing the slot
    pub async fn close(&self) {
        *self.is_closing.write().await = true;
        self.check_if_closing().await;
    }

    /// Check if we can close the slot
    async fn check_if_closing(&self) {
        let is_closing = *self.is_closing.read().await;
        if is_closing && self.is_idle().await {
            info!("Slot {} is now closed", self.name);
        }
    }

    /// Get the queue length
    pub async fn queue_length(&self) -> usize {
        self.queue.read().await.len()
    }

    /// Get the number of active requests
    pub async fn active_count(&self) -> usize {
        self.active.read().await.len()
    }

    /// Get the current active size
    pub async fn active_size(&self) -> usize {
        *self.active_size.read().await
    }
}

/// Slot manager for handling multiple slots
pub struct SlotManager {
    /// Map of slots by name
    slots: Arc<RwLock<HashMap<String, Arc<Slot>>>>,

    /// Default max active size for new slots
    default_max_active_size: usize,

    /// Default delay for new slots
    default_delay: u64,

    /// Scheduler factory for creating schedulers for new slots
    #[allow(clippy::type_complexity)]
    scheduler_factory: Option<Box<dyn Fn(String) -> Arc<dyn Scheduler> + Send + Sync>>,
}

impl SlotManager {
    /// Create a new slot manager
    pub fn new(default_max_active_size: usize, default_delay: u64) -> Self {
        Self {
            slots: Arc::new(RwLock::new(HashMap::new())),
            default_max_active_size,
            default_delay,
            scheduler_factory: None,
        }
    }

    /// Set the scheduler factory
    pub fn with_scheduler_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn(String) -> Arc<dyn Scheduler> + Send + Sync + 'static,
    {
        self.scheduler_factory = Some(Box::new(factory));
        self
    }

    /// Get or create a slot for a domain
    pub async fn get_slot(&self, domain: &str) -> Arc<Slot> {
        let mut slots = self.slots.write().await;

        if let Some(slot) = slots.get(domain) {
            slot.clone()
        } else {
            let slot = if let Some(factory) = &self.scheduler_factory {
                let scheduler = factory(domain.to_string());
                info!("Creating slot for domain {} with custom scheduler", domain);
                Arc::new(Slot::with_scheduler(
                    domain.to_string(),
                    self.default_max_active_size,
                    self.default_delay,
                    scheduler,
                ))
            } else {
                info!(
                    "Creating slot for domain {} with default FIFO queue",
                    domain
                );
                Arc::new(Slot::new(
                    domain.to_string(),
                    self.default_max_active_size,
                    self.default_delay,
                ))
            };

            slots.insert(domain.to_string(), slot.clone());
            slot
        }
    }

    /// Get all slot names
    pub async fn get_slot_names(&self) -> Vec<String> {
        let slots = self.slots.read().await;
        slots.keys().cloned().collect()
    }

    /// Get a specific slot
    pub async fn get_slot_by_name(&self, name: &str) -> Option<Arc<Slot>> {
        let slots = self.slots.read().await;
        slots.get(name).cloned()
    }

    /// Close a specific slot
    pub async fn close_slot(&self, name: &str) {
        if let Some(slot) = self.get_slot_by_name(name).await {
            slot.close().await;
        }
    }

    /// Close all slots
    pub async fn close_all_slots(&self) {
        let slot_names = self.get_slot_names().await;
        for name in slot_names {
            self.close_slot(&name).await;
        }
    }

    /// Check if all slots are idle
    pub async fn all_slots_idle(&self) -> bool {
        let slots = self.slots.read().await;
        for slot in slots.values() {
            if !slot.is_idle().await {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scrapy_rs_core::async_trait;
    use std::collections::HashSet;
    use std::time::Duration;

    #[tokio::test]
    async fn test_slot_basic() {
        let slot = Slot::new("test".to_string(), 10000, 100);

        // Check initial state
        assert_eq!(slot.name(), "test");
        assert_eq!(slot.queue_length().await, 0);
        assert_eq!(slot.active_count().await, 0);
        assert!(slot.is_idle().await);

        // Add a request
        let request = Request::get("https://example.com").unwrap();

        let _response_rx = slot.add_request(request.clone()).await;

        // Check queue
        assert_eq!(slot.queue_length().await, 1);
        assert!(!slot.is_idle().await);

        // Get the request
        let next = slot.next_request().await;
        assert!(next.is_some());

        // Check active
        assert_eq!(slot.active_count().await, 1);
        assert_eq!(slot.queue_length().await, 0);

        // Finish request
        let request_id = create_request_id(&request);
        slot.finish_request(&request_id, Some(2000)).await;

        // Check final state
        assert_eq!(slot.active_count().await, 0);
        assert!(slot.is_idle().await);
    }

    #[tokio::test]
    async fn test_slot_delay() {
        let slot = Slot::new("test".to_string(), 10000, 100);

        // Add two requests
        let request1 = Request::get("https://example.com/1").unwrap();

        let request2 = Request::get("https://example.com/2").unwrap();

        let _response_rx1 = slot.add_request(request1.clone()).await;
        let _response_rx2 = slot.add_request(request2.clone()).await;

        // Get the first request, which should be immediate
        let start = Instant::now();
        let next1 = slot.next_request().await;
        assert!(next1.is_some());
        let immediate_duration = start.elapsed();

        // Get the second request, which should be delayed
        let start = Instant::now();
        let next2 = slot.next_request().await;
        assert!(next2.is_some());
        let delayed_duration = start.elapsed();

        // Check that the second request was delayed
        assert!(delayed_duration >= Duration::from_millis(100));
        assert!(immediate_duration < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_slot_throttle() {
        let slot = Slot::new("test".to_string(), 2000, 0);

        // Add a request
        let request = Request::get("https://example.com").unwrap();

        let _response_rx = slot.add_request(request.clone()).await;

        // Get the request
        let next = slot.next_request().await;
        assert!(next.is_some());

        // Finish with a large response
        let request_id = create_request_id(&request);
        slot.finish_request(&request_id, Some(3000)).await;

        // Should need throttling
        assert!(slot.needs_throttle().await);
    }

    #[tokio::test]
    async fn test_slot_with_scheduler() {
        // Create a basic implementation of a scheduler for testing
        struct PriorityScheduler {
            requests: RwLock<Vec<Request>>,
            seen: RwLock<HashSet<String>>,
        }

        impl PriorityScheduler {
            fn new() -> Self {
                Self {
                    requests: RwLock::new(Vec::new()),
                    seen: RwLock::new(HashSet::new()),
                }
            }
        }

        #[async_trait]
        impl Scheduler for PriorityScheduler {
            async fn enqueue(&self, request: Request) -> Result<()> {
                let mut requests = self.requests.write().await;
                requests.push(request);
                // Sort by priority (higher first)
                requests.sort_by(|a, b| b.priority.cmp(&a.priority));
                Ok(())
            }

            async fn next(&self) -> Option<Request> {
                let mut requests = self.requests.write().await;
                if requests.is_empty() {
                    None
                } else {
                    Some(requests.remove(0))
                }
            }

            async fn is_empty(&self) -> bool {
                self.requests.read().await.is_empty()
            }

            async fn len(&self) -> usize {
                self.requests.read().await.len()
            }

            async fn has_seen(&self, url: &str) -> bool {
                self.seen.read().await.contains(url)
            }

            async fn mark_seen(&self, url: &str) -> Result<()> {
                self.seen.write().await.insert(url.to_string());
                Ok(())
            }

            async fn clear(&self) -> Result<()> {
                self.requests.write().await.clear();
                self.seen.write().await.clear();
                Ok(())
            }
        }

        // Create our test scheduler that prioritizes by priority field
        let scheduler = Arc::new(PriorityScheduler::new());

        // Create a slot with the scheduler
        let slot = Slot::with_scheduler("test-domain.com".to_string(), 10000, 0, scheduler.clone());

        // Create test requests with different priorities
        let low_priority = Request::get("https://test-domain.com/low")
            .unwrap()
            .with_priority(1);

        let high_priority = Request::get("https://test-domain.com/high")
            .unwrap()
            .with_priority(10);

        // Add requests to the slot in "wrong" order
        let _rx1 = slot.add_request(low_priority.clone()).await;
        let _rx2 = slot.add_request(high_priority.clone()).await;

        // Get the first request - should be high priority
        let next = slot.next_request().await;
        assert!(next.is_some());

        // Verify it's the high priority request
        if let Some((req, _)) = next {
            assert!(req.url.as_str().contains("/high"));
            assert_eq!(req.priority, 10);
        } else {
            panic!("Expected to get a request");
        }

        // Get the second request - should be low priority
        let next = slot.next_request().await;
        assert!(next.is_some());

        // Verify it's the low priority request
        if let Some((req, _)) = next {
            assert!(req.url.as_str().contains("/low"));
            assert_eq!(req.priority, 1);
        } else {
            panic!("Expected to get a request");
        }

        // Should be empty now
        let next = slot.next_request().await;
        assert!(next.is_none());
    }
}
