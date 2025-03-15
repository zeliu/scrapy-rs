# Scrapy-RS Scheduler

The scheduler component for the Scrapy-RS web crawler. It provides different scheduling strategies for crawling.

## Features

- **Multiple Scheduler Types**:
  - `MemoryScheduler`: Basic priority-based scheduler
  - `DomainGroupScheduler`: Groups requests by domain with per-domain rate limiting
  - `BreadthFirstScheduler`: Implements breadth-first search (BFS) crawling strategy
  - `DepthFirstScheduler`: Implements depth-first search (DFS) crawling strategy

- **Domain-aware Scheduling**:
  - Per-domain request rate limiting
  - Domain whitelist/blacklist support
  - Configurable delay between requests to the same domain

- **Flexible Crawl Strategies**:
  - Priority-based: Process requests based on their priority
  - Breadth-first: Process URLs level by level (good for wide crawls)
  - Depth-first: Process URLs by following paths deeply (good for deep crawls)

- **Depth Control**:
  - Maximum depth configuration
  - Depth-based prioritization

## Usage

### Basic Usage

```rust
use scrapy_rs::{MemoryScheduler, Scheduler};
use scrapy_rs::request::Request;

#[tokio::main]
async fn main() {
    // Create a scheduler
    let scheduler = MemoryScheduler::new();
    
    // Create and enqueue a request
    let request = Request::get("https://example.com").unwrap();
    scheduler.enqueue(request).await.unwrap();
    
    // Get the next request
    if let Some(next_request) = scheduler.next().await {
        println!("Next request: {}", next_request.url);
    }
}
```

### Domain-aware Scheduling

```rust
use scrapy_rs_scheduler::{DomainGroupScheduler, SchedulerConfig, CrawlStrategy};
use scrapy_rs_core::request::Request;
use std::collections::HashSet;

#[tokio::main]
async fn main() {
    // Create a domain whitelist
    let mut whitelist = HashSet::new();
    whitelist.insert("example.com".to_string());
    
    // Create scheduler configuration
    let config = SchedulerConfig {
        strategy: CrawlStrategy::Priority,
        max_requests_per_domain: Some(10),
        domain_delay_ms: Some(1000), // 1 second delay between requests to same domain
        domain_whitelist: Some(whitelist),
        domain_blacklist: None,
        respect_depth: true,
        max_depth: Some(3),
    };
    
    // Create a domain-aware scheduler
    let scheduler = DomainGroupScheduler::new(config);
    
    // Use the scheduler...
}
```

### Breadth-First Search (BFS)

```rust
use scrapy_rs_scheduler::{BreadthFirstScheduler, Scheduler};
use scrapy_rs_core::request::Request;

#[tokio::main]
async fn main() {
    // Create a BFS scheduler with max depth of 3
    let scheduler = BreadthFirstScheduler::new(Some(3));
    
    // Use the scheduler...
}
```

### Depth-First Search (DFS)

```rust
use scrapy_rs_scheduler::{DepthFirstScheduler, Scheduler};
use scrapy_rs_core::request::Request;

#[tokio::main]
async fn main() {
    // Create a DFS scheduler with max depth of 5
    let scheduler = DepthFirstScheduler::new(Some(5));
    
    // Use the scheduler...
}
```

## Integration with Engine

When using the Scrapy-RS engine, you can configure the scheduler type and strategy:

```rust
use scrapy_rs_engine::{Engine, EngineConfig, SchedulerType, CrawlStrategy};
use scrapy_rs_core::spider::Spider;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Create a spider
    let spider = Arc::new(MySpider::new());
    
    // Create engine configuration with domain-aware scheduler
    let config = EngineConfig {
        scheduler_type: SchedulerType::DomainGroup,
        crawl_strategy: CrawlStrategy::BreadthFirst,
        max_depth: Some(3),
        domain_delay_ms: Some(500),
        max_requests_per_domain: Some(10),
        ..EngineConfig::default()
    };
    
    // Create and run the engine
    let mut engine = Engine::new(spider).unwrap();
    engine = engine.with_config(config);
    engine.run().await.unwrap();
}
```

## Example

See the `scheduler_example.rs` in the examples directory for a complete example of using different scheduler types and strategies. 