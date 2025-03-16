# Scrapy-RS

Python bindings for the Scrapy-RS web crawler, a high-performance web crawler written in Rust.

## Installation

You can install Scrapy-RS using pip:

```bash
pip install scrapy-rs
```

### Binary Wheels

Pre-built binary wheels are available for:
- Windows (x86_64)
- macOS (x86_64, arm64)
- Linux (x86_64)

For other platforms, pip will attempt to build from source, which requires:
- Rust compiler (1.70+)
- Python development headers (3.9+)

## Usage

### Creating a Project

```python
import scrapy_rs

# Create a new project
scrapy_rs.startproject('myproject', '/path/to/directory')
```

### Creating a Spider

```python
import scrapy_rs

# Create a new spider
scrapy_rs.genspider('myspider', 'example.com')
```

### Running a Spider

```python
import scrapy_rs

# Run a spider
scrapy_rs.crawl('myspider')
```

## Features

- High-performance web crawling powered by Rust
- Python API compatible with Scrapy
- Automatic fallback to Python implementation if Rust extension is not available
- Support for custom middleware and pipelines

 