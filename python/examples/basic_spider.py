#!/usr/bin/env python3
"""
Basic example of using RS-Spider from Python.
This example creates a simple spider that crawls a website and extracts some data.
"""

import sys
import os
import time

# Add parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
try:
    from src.rs_spider import PySpider, PyEngine, PyItem, PyRequest
except ImportError:
    print("Unable to import rs_spider module. Please make sure Python bindings are properly installed.")
    print("Current Python path:", sys.path)
    sys.exit(1)

class BasicSpider:
    """A simple spider that crawls a website and extracts some data."""
    
    def __init__(self, name, start_urls, allowed_domains=None):
        """Initialize the spider with a name, start URLs, and optional allowed domains."""
        self.spider = PySpider(name, start_urls, allowed_domains)
        self.engine = PyEngine(self.spider)
        
    def parse(self, response):
        """Parse a response and extract data."""
        print(f"Parsing {response.url} (status: {response.status})")
        
        # Create an item to store the data
        item = PyItem("webpage")
        item.set("url", response.url)
        item.set("title", "Example Title")  # In a real spider, you would extract this from the response
        item.set("timestamp", time.time())
        
        # Create new requests to follow
        new_requests = []
        # In a real spider, you would extract links from the response and create requests for them
        
        return [item], new_requests
    
    def run(self):
        """Run the spider and print the results."""
        print(f"Starting spider: {self.spider.name}")
        print(f"Start URLs: {self.spider.start_urls}")
        print(f"Allowed domains: {self.spider.allowed_domains}")
        
        # Run the engine
        stats = self.engine.run()
        
        # Print the results
        print("\nCrawl completed!")
        print(f"Requests: {stats.request_count}")
        print(f"Responses: {stats.response_count}")
        print(f"Items: {stats.item_count}")
        print(f"Errors: {stats.error_count}")
        print(f"Duration: {stats.duration_seconds:.2f} seconds")
        print(f"Requests per second: {stats.requests_per_second:.2f}")


def main():
    """Run the example spider."""
    # Create a spider
    spider = BasicSpider(
        name="example",
        start_urls=["https://example.com"],
        allowed_domains=["example.com"]
    )
    
    # Run the spider
    spider.run()


if __name__ == "__main__":
    main() 