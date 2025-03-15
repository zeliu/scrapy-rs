# Scrapy-RS Design Document

## Overview

Scrapy-RS is a high-performance web crawler framework written in Rust, designed to be compatible with Python's Scrapy while leveraging Rust's performance benefits. This document outlines the architecture, design principles, and implementation details of the Scrapy-RS project.

## Goals

1. **Performance**: Achieve significantly better performance than Python's Scrapy.
2. **Compatibility**: Provide a familiar API for Scrapy users.
3. **Extensibility**: Allow easy extension and customization.
4. **Reliability**: Handle errors gracefully and provide robust crawling capabilities.
5. **Scalability**: Support distributed crawling and horizontal scaling.

## Architecture

Scrapy-RS follows a modular architecture with the following core components:

1. **Core**: Contains the fundamental data structures and traits used throughout the framework.
   - `Request`: Represents an HTTP request.
   - `Response`: Represents an HTTP response.
   - `Item`: Represents a scraped item.
   - `Spider`: Trait for defining spiders.
   - `Error`: Error handling types.

2. **Downloader**: Responsible for making HTTP requests and handling responses.
   - `Downloader`: Trait for downloading content.
   - `HttpDownloader`: Implementation using reqwest.
   - `DownloaderMiddleware`: For modifying requests and responses.

3. **Scheduler**: Manages the request queue and prioritization.
   - `Scheduler`: Trait for scheduling requests.
   - `MemoryScheduler`: In-memory implementation.
   - `RedisScheduler`: Redis-based implementation for distributed crawling.

4. **Middleware**: Provides hooks for modifying the behavior of the crawler.
   - `SpiderMiddleware`: For processing spider input and output.
   - `DownloaderMiddleware`: For processing requests and responses.

5. **Pipeline**: Processes scraped items.
   - `Pipeline`: Trait for processing items.
   - `JsonFilePipeline`: Saves items to a JSON file.
   - `CsvFilePipeline`: Saves items to a CSV file.

6. **Engine**: Coordinates the other components and manages the crawling process.
   - `Engine`: Main engine that orchestrates the crawling process.
   - `EngineConfig`: Configuration for the engine.
   - `EngineStats`: Statistics about the crawling process.

7. **Settings**: Manages configuration settings.
   - `Settings`: Stores and retrieves configuration values.
   - `SettingsLoader`: Loads settings from various sources.

8. **Python Bindings**: Provides Python bindings for the framework.
   - `PySpider`: Python wrapper for the Spider trait.
   - `PyEngine`: Python wrapper for the Engine.
   - `PyItem`: Python wrapper for the Item struct.

## Data Flow

1. The `Engine` starts the crawling process by getting the start URLs from the `Spider`.
2. The `Engine` creates `Request` objects for each start URL and sends them to the `Scheduler`.
3. The `Scheduler` prioritizes and queues the requests.
4. The `Engine` gets the next request from the `Scheduler` and sends it to the `Downloader`.
5. The `Downloader` makes the HTTP request and returns a `Response`.
6. The `Engine` sends the `Response` to the `Spider` for parsing.
7. The `Spider` extracts data from the `Response` and returns `Item`s and new `Request`s.
8. The `Engine` sends the `Item`s to the `Pipeline` for processing.
9. The `Engine` sends the new `Request`s to the `Scheduler`.
10. The process repeats until there are no more requests or the crawling is stopped.

## Concurrency Model

Scrapy-RS uses Rust's async/await for concurrency:

1. The `Engine` uses a task pool to process multiple requests concurrently.
2. The `Downloader` uses async HTTP clients to make non-blocking requests.
3. The `Scheduler` is thread-safe and can be accessed from multiple tasks.
4. The `Pipeline` processes items concurrently.

## Error Handling

Scrapy-RS uses a comprehensive error handling system:

1. Each component defines its own error types that implement the `Error` trait.
2. Errors are propagated up the call stack using Rust's `Result` type.
3. The `Engine` handles errors by logging them and optionally retrying the request.
4. Middleware can intercept and handle errors.

## Configuration

Scrapy-RS is highly configurable:

1. The `Settings` component provides a centralized configuration system.
2. Settings can be loaded from various sources (environment variables, files, etc.).
3. Each component has sensible defaults but can be configured.
4. Configuration can be done programmatically or through configuration files.

## Python Bindings

Scrapy-RS provides Python bindings using PyO3:

1. The `PySpider` class wraps the Rust `Spider` trait.
2. The `PyEngine` class wraps the Rust `Engine` struct.
3. The `PyItem` class wraps the Rust `Item` struct.
4. Python callbacks can be registered for various events.
5. Python code can extend and customize the behavior of the crawler.

## Performance Optimizations

1. **Memory Efficiency**: Minimize allocations and use efficient data structures.
2. **CPU Efficiency**: Use Rust's zero-cost abstractions and avoid unnecessary computations.
3. **I/O Efficiency**: Use non-blocking I/O and connection pooling.
4. **Concurrency**: Process multiple requests concurrently.
5. **Caching**: Cache responses to avoid redundant requests.

## Conclusion

Scrapy-RS combines the performance benefits of Rust with the ease of use of Python's Scrapy framework. Its modular architecture makes it easy to extend and customize, while its async/await concurrency model provides excellent performance. The Python bindings make it accessible to Python developers, while the Rust core provides the performance benefits. 