# Scrapy-RS

Scrapy-RS is a high-performance web crawler written in Rust, with Python bindings. It is designed to be fast, memory-efficient, and easy to use.

## Features

- **High Performance**: Written in Rust for maximum speed and memory efficiency.
- **Concurrent**: Uses async/await for high concurrency without the overhead of threads.
- **Memory Safe**: Leverages Rust's memory safety guarantees to prevent common bugs.
- **Modular**: Designed with a modular architecture for easy extension and customization.
- **Python Bindings**: Can be used from Python via PyO3 bindings.
- **Scrapy-Compatible**: Familiar API for users of the popular Python Scrapy framework.
- **Benchmarking Tools**: Includes tools for comparing performance with Python's Scrapy, with a mock server for controlled testing environments.

## Components

Scrapy-RS is composed of several components:

- **Core**: The core library that defines the basic types and traits.
- **Scheduler**: Manages the queue of URLs to crawl.
- **Downloader**: Handles HTTP requests and responses.
- **Pipeline**: Processes scraped items.
- **Middleware**: Hooks for modifying requests and responses.
- **Engine**: Orchestrates the crawling process.
- **Python**: Python bindings for the crawler.
- **Benchmark**: Tools for performance comparison between Scrapy-RS and Python's Scrapy, featuring a mock server for controlled and reproducible testing.

## Installation

### Rust

To use Scrapy-RS in a Rust project, add it to your `Cargo.toml`:

```toml
[dependencies]
scrapy_rs = "0.1.0"
```

### Python

To use Scrapy-RS from Python, install it using pip:

```bash
pip install scrapy_rs
```

Or, if you want to install from source:

```bash
git clone https://github.com/liuze/scrapy_rs.git
cd scrapy_rs/python
pip install -e .
```

## Usage

### Rust

Here's a simple example of how to use Scrapy-RS in Rust:

```rust
use scrapy_rs_core::request::Request;
use scrapy_rs_core::spider::{BasicSpider, Spider};
use scrapy_rs_engine::Engine;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a spider
    let spider = Arc::new(BasicSpider::new(
        "example",
        vec!["https://example.com".to_string()],
    ));

    // Create an engine
    let mut engine = Engine::new(spider).await?;

    // Run the engine
    let stats = engine.run().await?;

    // Print the results
    println!("Requests: {}", stats.request_count);
    println!("Responses: {}", stats.response_count);
    println!("Items: {}", stats.item_count);
    println!("Errors: {}", stats.error_count);
    println!("Duration: {:?}", stats.duration());
    println!("Requests per second: {:?}", stats.requests_per_second());

    Ok(())
}
```

### Python

Here's a simple example of how to use Scrapy-RS from Python:

```python
from scrapy_rs import PySpider, PyEngine, PyItem, PyRequest

# Create a spider
spider = PySpider(
    name="example",
    start_urls=["https://example.com"],
    allowed_domains=["example.com"]
)

# Create an engine
engine = PyEngine(spider)

# Run the engine
stats = engine.run()

# Print the results
print(f"Requests: {stats.request_count}")
print(f"Responses: {stats.response_count}")
print(f"Items: {stats.item_count}")
print(f"Errors: {stats.error_count}")
print(f"Duration: {stats.duration_seconds:.2f} seconds")
print(f"Requests per second: {stats.requests_per_second:.2f}")
```

### Benchmarking

Here's how to run benchmarks to compare Scrapy-RS and Scrapy performance:

```bash
# Simple benchmark with real websites
cd benchmark
cargo run -- run --urls https://example.com --output-dir results

# Benchmark with mock server for controlled testing environment
cargo run -- run --use-mock-server --mock-server-pages 50 --mock-server-links 5 --output-dir results_mock

# Generate HTML report comparing the results
cargo run -- report --input-dir results --output-dir reports
```

For more advanced usage, see the examples in the `examples` directory and the [Benchmark Documentation](benchmark/README.md).

## Documentation

For more detailed documentation, see the [docs](docs/) directory.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details. 