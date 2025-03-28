# Scrapy-RS Benchmark

This module provides benchmarking tools for comparing the performance of Scrapy-RS with Python's Scrapy.

## Features

- Compare Scrapy-RS and Scrapy performance on the same crawling tasks
- Multiple predefined benchmark scenarios
- Custom benchmark scenarios
- Detailed performance metrics
- HTML reports with charts
- Criterion-based micro-benchmarks
- Mock server for controlled and reproducible testing environments

## Requirements

- Rust (stable)
- Python 3.7+
- Scrapy (for comparison benchmarks)

## Installation

First, make sure you have Scrapy installed:

```bash
pip install scrapy
```

Then, build the benchmark tool:

```bash
cd benchmark
cargo build --release
```

## Usage

### List Available Scenarios

```bash
cargo run --release -- list
```

### Run a Benchmark

Run a predefined scenario:

```bash
cargo run --release -- run --scenario simple --output-dir results
```

Run a custom benchmark:

```bash
cargo run --release -- run --urls https://example.com --page-limit 100 --max-depth 2 --concurrent-requests 8 --output-dir results
```

Run only Scrapy-RS:

```bash
cargo run --release -- run --scenario medium --only-scrapy-rs --output-dir results
```

Run only Scrapy:

```bash
cargo run --release -- run --scenario medium --only-scrapy --output-dir results
```

### Using the Mock Server

Run a benchmark with the mock server:

```bash
cargo run --release -- run --use-mock-server --output-dir results
```

Customize the mock server configuration:

```bash
cargo run --release -- run --use-mock-server --mock-server-pages 50 --mock-server-links 5 --output-dir results
```

For more details about the mock server, see [Mock Server Documentation](README_MOCK_SERVER.md).

### Generate a Report

Generate a report from existing benchmark results:

```bash
cargo run --release -- report --input-dir results --output-dir reports
```

### Run Criterion Benchmarks

Run the Criterion-based micro-benchmarks:

```bash
cargo bench
```

## Benchmark Scenarios

The following predefined scenarios are available:

1. **simple**: A simple crawl of a few pages from example.com
2. **medium**: A medium-sized crawl of quotes.toscrape.com with moderate depth
3. **complex**: A complex crawl with multiple start URLs and high concurrency
4. **real_world**: A real-world crawl of news.ycombinator.com with rate limiting

## Metrics

The benchmark collects the following metrics:

- **Duration**: Total time taken for the crawl
- **Request count**: Number of requests made
- **Response count**: Number of responses received
- **Item count**: Number of items scraped
- **Error count**: Number of errors encountered
- **Requests per second**: Rate of requests
- **Items per second**: Rate of items scraped
- **Memory usage**: Memory consumption in MB
- **CPU usage**: CPU utilization percentage

## Report Format

The HTML report includes:

- Performance comparison charts
- Resource usage charts
- Detailed results table

## Mock Server

The benchmark tool includes a mock server that can be used to create a controlled and reproducible environment for testing. Using the mock server has several advantages:

- **Reproducibility**: Tests can be run in a controlled environment without external dependencies
- **Consistency**: Eliminates variables like network latency and website response times
- **Accurate Comparison**: Provides a fair comparison between different crawlers
- **No External Impact**: Avoids impacting real websites with benchmark traffic
- **Configurable**: Allows customization of the test environment (number of pages, links, response delays, etc.)

The mock server simulates a website with configurable parameters such as:
- Number of pages
- Links per page
- Response delay
- Failure simulation

See the [Mock Server Documentation](README_MOCK_SERVER.md) for detailed information on how to use and configure the mock server.

## Extending

### Adding New Scenarios

To add a new benchmark scenario, edit `src/common.rs` and add your scenario to the `get_predefined_scenarios` function.

### Adding New Metrics

To add new metrics, modify the `BenchmarkResult` struct in `src/lib.rs` and update the report generation code.

## License

This project is licensed under the MIT License - see the LICENSE file for details. 