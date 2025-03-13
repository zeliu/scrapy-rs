# RS-Spider Python Bindings

This package provides Python bindings for the RS-Spider web crawler, a high-performance web crawler written in Rust.

## Installation

To install the package, you need to have Rust and Python installed on your system. Then, you can install the package using pip:

```bash
pip install rs-spider
```

Or, if you want to install from source:

```bash
git clone https://github.com/yourusername/rs-spider.git
cd rs-spider/python
pip install -e .
```

## Usage

Here's a simple example of how to use the RS-Spider from Python:

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

For more advanced usage, see the examples in the `examples` directory.

## API Reference

### PySpider

A wrapper for the Rust `Spider` trait.

```python
PySpider(name, start_urls, allowed_domains=None)
```

- `name`: The name of the spider.
- `start_urls`: A list of URLs to start crawling from.
- `allowed_domains`: An optional list of domains that the spider is allowed to crawl.

#### Properties

- `name`: The name of the spider.
- `allowed_domains`: The domains that the spider is allowed to crawl.
- `start_urls`: The URLs to start crawling from.

#### Methods

- `parse(response)`: Parse a response and return a tuple of `(items, requests)`.

### PyEngine

A wrapper for the Rust `Engine` struct.

```python
PyEngine(spider)
```

- `spider`: A `PySpider` instance.

#### Methods

- `run()`: Run the engine and return a `PyEngineStats` instance.
- `stats()`: Get the current engine statistics.
- `is_running()`: Check if the engine is running.

### PyRequest

A wrapper for the Rust `Request` struct.

```python
PyRequest(url, method=None, headers=None, body=None)
```

- `url`: The URL to request.
- `method`: The HTTP method to use (default: "GET").
- `headers`: A dictionary of HTTP headers.
- `body`: The request body as bytes.

#### Properties

- `url`: The URL of the request.
- `method`: The HTTP method of the request.
- `headers`: The HTTP headers of the request.
- `body`: The body of the request.
- `meta`: The metadata of the request.

#### Methods

- `set_meta(key, value)`: Set metadata for the request.
- `set_callback(callback)`: Set the callback for the request.
- `set_errback(errback)`: Set the errback for the request.
- `set_priority(priority)`: Set the priority for the request.

### PyResponse

A wrapper for the Rust `Response` struct.

#### Properties

- `url`: The URL of the response.
- `status`: The HTTP status code of the response.
- `headers`: The HTTP headers of the response.
- `body`: The body of the response as bytes.
- `request`: The request that generated this response.
- `meta`: The metadata of the response.

#### Methods

- `text()`: Get the body of the response as text.
- `json()`: Get the body of the response as JSON.
- `is_success()`: Check if the response was successful.
- `is_redirect()`: Check if the response is a redirect.

### PyItem

A wrapper for the Rust `DynamicItem` struct.

```python
PyItem(item_type)
```

- `item_type`: The type of the item.

#### Properties

- `item_type`: The type of the item.
- `fields`: A dictionary of all fields in the item.

#### Methods

- `get(key)`: Get a field value.
- `set(key, value)`: Set a field value.
- `has_field(key)`: Check if a field exists.

### PyEngineStats

A wrapper for the Rust `EngineStats` struct.

#### Properties

- `request_count`: The number of requests sent.
- `response_count`: The number of responses received.
- `item_count`: The number of items scraped.
- `error_count`: The number of errors.
- `duration_seconds`: The duration of the crawl in seconds.
- `requests_per_second`: The requests per second.

## Examples

See the `examples` directory for more examples of how to use the RS-Spider from Python.

## License

This project is licensed under the MIT License - see the LICENSE file for details. 