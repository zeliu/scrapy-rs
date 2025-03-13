//! # Scrapy-RS
//!
//! Scrapy-RS is a high-performance web crawler written in Rust, with Python bindings.
//! It is designed to be fast, memory-efficient, and easy to use.
//!
//! ## Features
//!
//! - **High Performance**: Written in Rust for maximum speed and memory efficiency.
//! - **Concurrent**: Uses async/await for high concurrency without the overhead of threads.
//! - **Memory Safe**: Leverages Rust's memory safety guarantees to prevent common bugs.
//! - **Modular**: Designed with a modular architecture for easy extension and customization.
//! - **Python Bindings**: Can be used from Python via PyO3 bindings.
//! - **Scrapy-Compatible**: Familiar API for users of the popular Python Scrapy framework.
//!
//! ## Components
//!
//! Scrapy-RS is composed of several components:
//!
//! - **Core**: The core library that defines the basic types and traits.
//! - **Scheduler**: Manages the queue of URLs to crawl.
//! - **Downloader**: Handles HTTP requests and responses.
//! - **Pipeline**: Processes scraped items.
//! - **Middleware**: Hooks for modifying requests and responses.
//! - **Engine**: Orchestrates the crawling process.
//! - **Python**: Python bindings for the crawler.
//! - **Settings**: Configuration management for the crawler.
//!
//! ## Example
//!
//! ```rust,no_run
//! use scrapy_rs::prelude::*;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Initialize logger
//!     env_logger::init();
//!
//!     // Create a spider
//!     let spider = Arc::new(BasicSpider::new(
//!         "example",
//!         vec!["https://example.com".to_string()],
//!     ));
//!
//!     // Create an engine
//!     let mut engine = Engine::new(spider.clone())?;
//!
//!     // Run the engine
//!     let stats = engine.run().await?;
//!
//!     // Print the results
//!     println!("Crawl completed!");
//!     println!("Requests: {}", stats.request_count);
//!     println!("Responses: {}", stats.response_count);
//!     println!("Items: {}", stats.item_count);
//!     println!("Errors: {}", stats.error_count);
//!
//!     Ok(())
//! }
//! ```

pub use scrapy_rs_core as core;
pub use scrapy_rs_downloader as downloader;
pub use scrapy_rs_engine as engine;
pub use scrapy_rs_middleware as middleware;
pub use scrapy_rs_pipeline as pipeline;
pub use scrapy_rs_scheduler as scheduler;

// Settings module for configuration management
pub mod settings;

// Config adapters module
pub mod config_adapters;

/// Prelude module that re-exports commonly used types
pub mod prelude {
    pub use scrapy_rs_core::error::{Error, Result};
    pub use scrapy_rs_core::item::{DynamicItem, Item};
    pub use scrapy_rs_core::request::{Method, Request};
    pub use scrapy_rs_core::response::Response;
    pub use scrapy_rs_core::spider::{BasicSpider, ParseOutput, Spider};
    pub use scrapy_rs_downloader::{Downloader, DownloaderConfig, HttpDownloader};
    pub use scrapy_rs_engine::{Engine, EngineConfig, EngineStats, SchedulerType};
    pub use scrapy_rs_middleware::{
        ChainedRequestMiddleware, ChainedResponseMiddleware, DefaultHeadersMiddleware,
        RandomDelayMiddleware, RequestMiddleware, ResponseLoggerMiddleware, ResponseMiddleware,
        RetryMiddleware, UrlFilterMiddleware,
    };
    pub use scrapy_rs_pipeline::{
        DummyPipeline, FilterPipeline, JsonFilePipeline, LogPipeline, Pipeline, PipelineType,
    };
    pub use scrapy_rs_scheduler::{
        BreadthFirstScheduler, CrawlStrategy, DepthFirstScheduler, DomainGroupScheduler,
        MemoryScheduler, Scheduler, SchedulerConfig,
    };

    // Export settings module
    pub use crate::settings::{Settings, SettingsError, SettingsFormat};
}
