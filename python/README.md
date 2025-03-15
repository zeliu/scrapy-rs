# Scrapy-RS Python Bindings

Python bindings for Scrapy-RS, a high-performance web crawler written in Rust.

## Overview

Scrapy-RS is a web crawling and scraping framework written in Rust, designed for high performance and efficiency. These Python bindings allow you to use Scrapy-RS from Python, combining the speed of Rust with the ease of use and extensive ecosystem of Python.

## Features

- High-performance web crawling powered by Rust
- Familiar Python API similar to Scrapy
- Fallback implementation for environments where the Rust extension cannot be built
- Seamless integration with Python's async/await syntax

## Installation

For detailed installation instructions, please refer to:

- [Comprehensive Installation Guide](INSTALL.md) - Complete guide with explanations and troubleshooting
- [Quick Installation Guide](QUICK_INSTALL.md) - Streamlined steps for quick setup

## Basic Usage

```python
import scrapy_rs

# Print the version
print(scrapy_rs.__version__)

# Use the hello function
greeting = scrapy_rs.hello()
print(greeting)
```

## Project Structure

```
python/
├── Cargo.toml          # Rust crate configuration
├── setup.py            # Python package configuration
├── INSTALL.md          # Comprehensive installation guide
├── QUICK_INSTALL.md    # Quick installation guide
├── README.md           # This file
└── src/
    ├── lib.rs          # Rust implementation of Python bindings
    └── scrapy_rs/      # Python package
        └── __init__.py # Python module initialization
```

## Development

For development, it's recommended to install the package in development mode:

```bash
pip install -e .
```

This allows changes to the Python code to be immediately reflected without reinstallation.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [Scrapy](https://scrapy.org/) - The Python web crawling framework that inspired this project
- [PyO3](https://pyo3.rs/) - Rust bindings for Python used in this project 