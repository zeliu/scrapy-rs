# RS-Spider Design Document

## Overview

RS-Spider is a high-performance web crawler framework written in Rust, designed to be compatible with Python's Scrapy while leveraging Rust's performance benefits. This document outlines the architecture, design principles, and implementation details of the RS-Spider project.

## Design Goals

1. **High Performance**: Utilize Rust's concurrency model and memory safety features to create a crawler that is significantly faster and more resource-efficient than existing Python solutions.

2. **Scrapy Compatibility**: Provide an API that is familiar to users of Python's Scrapy framework, making it easy for them to migrate existing projects.

3. **Python Integration**: Allow users to write crawler logic in Python while leveraging the performance benefits of Rust through PyO3 bindings.

4. **Modularity**: Design the system with clear component boundaries to enable easy extension and customization.

5. **Memory Safety**: Leverage Rust's ownership model to prevent common bugs like memory leaks, data races, and null pointer dereferences.

6. **Concurrency**: Use Rust's async/await to handle many concurrent requests efficiently without the overhead of threads.

## Architecture

RS-Spider follows a modular architecture with the following core components:

### Core

The `core` module defines the fundamental data structures and traits used throughout the system:

- **Request**: Represents an HTTP request with URL, method, headers, body, and metadata.
- **Response**: Represents an HTTP response with status, headers, body, and the original request.
- **Item**: A trait for scraped data items with serialization/deserialization support.
- **Spider**: A trait that defines the crawler behavior, including start URLs and parsing logic.
- **Error**: A comprehensive error handling system using `thiserror`.

### Scheduler

The `scheduler` module manages the queue of URLs to crawl:

- **Scheduler Trait**: Defines the interface for all schedulers.
- **MemoryScheduler**: An in-memory implementation that uses a priority queue.
- **FifoScheduler**: A first-in-first-out implementation.
- URL deduplication to avoid crawling the same URL multiple times.
- Priority-based scheduling to control the order of crawling.

### Downloader

The `downloader` module handles HTTP requests and responses:

- **Downloader Trait**: Defines the interface for all downloaders.
- **HttpDownloader**: An implementation using `reqwest` for HTTP requests.
- Concurrent request handling with configurable limits.
- Retry logic for failed requests.
- Respect for robots.txt rules.
- Rate limiting to avoid overwhelming target servers.

### Pipeline

The `pipeline` module processes scraped data:

- **Pipeline Trait**: Defines the interface for all pipelines.
- **DummyPipeline**: A no-op pipeline for testing.
- **LogPipeline**: Logs items at a specified log level.
- **JsonFilePipeline**: Writes items to a JSON file.
- **FilterPipeline**: Filters items based on a predicate.
- **ChainedPipeline**: Chains multiple pipelines together.

### Middleware

The `middleware` module provides hooks for modifying requests and responses:

- **RequestMiddleware Trait**: For processing requests before they are sent.
- **ResponseMiddleware Trait**: For processing responses after they are received.
- **DefaultHeadersMiddleware**: Adds default headers to requests.
- **RandomDelayMiddleware**: Introduces random delays between requests.
- **UrlFilterMiddleware**: Filters requests based on URL patterns.
- **ResponseLoggerMiddleware**: Logs responses.
- **RetryMiddleware**: Retries failed requests.
- **ChainedMiddleware**: Chains multiple middlewares together.

### Engine

The `engine` module orchestrates the crawling process:

- Coordinates all components (spider, scheduler, downloader, pipelines, middlewares).
- Manages concurrency with configurable limits.
- Collects and reports statistics.
- Handles graceful shutdown.
- Provides a simple API for starting and stopping the crawler.

### Python Bindings

The `python` module provides Python bindings using PyO3:

- **PyRequest**: Python wrapper for `Request`.
- **PyResponse**: Python wrapper for `Response`.
- **PyItem**: Python wrapper for `DynamicItem`.
- **PySpider**: Python wrapper for `Spider`.
- **PyEngine**: Python wrapper for `Engine`.
- **PyEngineStats**: Python wrapper for `EngineStats`.
- Conversion utilities between Python and Rust data types.

## Data Flow

1. The **Spider** provides start URLs, which are converted to **Requests**.
2. **Requests** are processed by **RequestMiddleware** and then sent to the **Scheduler**.
3. The **Scheduler** queues the requests and deduplicates them.
4. The **Engine** pulls requests from the **Scheduler** and sends them to the **Downloader**.
5. The **Downloader** fetches the content and creates a **Response**.
6. **Responses** are processed by **ResponseMiddleware** and then sent to the **Spider** for parsing.
7. The **Spider** extracts data (**Items**) and new **Requests** from the **Response**.
8. **Items** are sent to the **Pipeline** for processing and storage.
9. New **Requests** are sent back to the **Scheduler**, continuing the cycle.

## Concurrency Model

RS-Spider uses Rust's async/await for concurrency:

- The **Engine** manages a pool of tasks for concurrent downloads.
- Tokio is used as the async runtime.
- Semaphores limit the number of concurrent requests.
- Channels are used for communication between components.
- Shared state is protected by Mutex and RwLock.

## Error Handling

RS-Spider uses a comprehensive error handling system:

- A central `Error` enum with variants for different error types.
- `thiserror` for deriving error implementations.
- Propagation of errors through the Result type.
- Conversion between Rust errors and Python exceptions in the Python bindings.

## Configuration

RS-Spider is highly configurable:

- **EngineConfig**: Controls the behavior of the engine, including concurrency limits, delays, and logging.
- **DownloaderConfig**: Controls the behavior of the downloader, including timeouts and retry logic.
- **Spider-specific settings**: Each spider can define its own settings.

## Python Integration

RS-Spider provides Python bindings using PyO3:

- Python users can define spiders in Python.
- The core crawling logic runs in Rust for maximum performance.
- Data is seamlessly converted between Python and Rust.
- The API is designed to be familiar to Scrapy users.

## Performance Considerations

- **Memory Efficiency**: Rust's ownership model ensures efficient memory usage.
- **CPU Efficiency**: Async/await provides concurrency without the overhead of threads.
- **I/O Efficiency**: Non-blocking I/O for network operations.
- **Caching**: URL deduplication to avoid redundant requests.
- **Parallelism**: Configurable concurrency limits to maximize throughput.

## Future Improvements

- **Distributed Crawling**: Support for distributed crawling across multiple machines.
- **Database Integration**: Built-in support for storing items in databases.
- **More Middleware**: Additional middleware for common tasks like JavaScript rendering.
- **More Pipelines**: Additional pipelines for common tasks like image downloading.
- **More Schedulers**: Additional schedulers for different scheduling strategies.
- **More Downloaders**: Additional downloaders for different protocols.
- **More Spiders**: Additional spider implementations for common use cases.
- **More Python Integration**: More comprehensive Python API.

## Conclusion

RS-Spider combines the performance benefits of Rust with the ease of use of Python's Scrapy framework. Its modular architecture makes it easy to extend and customize, while its async/await concurrency model ensures efficient resource usage. The Python bindings make it accessible to Python developers, while the Rust core ensures maximum performance. 