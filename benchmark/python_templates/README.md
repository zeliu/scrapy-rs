# Scrapy Benchmark Templates

This directory contains template files for the Scrapy benchmark tests. These files are used by the Rust benchmark runner to create a temporary Scrapy project for benchmarking.

## File Structure

- `__init__.py` - Makes the directory a Python package
- `items.py` - Defines the `BenchmarkItem` class for storing crawled data
- `pipelines.py` - Defines the `BenchmarkPipeline` class for processing and saving items
- `settings.py` - Contains the Scrapy project settings
- `main.py` - Entry point for running the benchmark
- `spiders/` - Directory containing spider definitions
  - `__init__.py` - Makes the directory a Python package
  - `benchmark_spider.py` - Defines the `BenchmarkSpider` class for crawling websites

## How It Works

The Rust benchmark runner copies these template files to a temporary directory and modifies them with the specific benchmark parameters (URLs, page limits, etc.) before running the benchmark.

## Manual Execution

You can also run the benchmark manually using the `main.py` script:

```bash
python main.py --stats-file stats.json --output-dir results
```

### Command-line Parameters

- `--stats-file` - Path to save the Scrapy stats in JSON format
- `--output-dir` - Directory to save the crawled items

## Note

These files are templates and are meant to be modified by the Rust code before execution. The placeholders in the files (like `self.start_urls = []`) will be replaced with actual values during the benchmark setup. 