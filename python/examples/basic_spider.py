#!/usr/bin/env python3
"""
Basic example of using Scrapy-RS from Python.
This example creates a simple spider that crawls a website and extracts some data.
"""

import sys
import os
import time

# Mock classes to simulate the Rust bindings
class PyRequest:
    def __init__(self, url, method="GET"):
        self.url = url
        self.method = method
        self.headers = {}
        self.meta = {}
    
    def set_meta(self, key, value):
        self.meta[key] = value
        return self

class PyResponse:
    def __init__(self, url, status=200, body=""):
        self.url = url
        self.status = status
        self.body = body
        self.headers = {}

class PyItem:
    def __init__(self, item_type):
        self.type = item_type
        self.fields = {}
    
    def set(self, key, value):
        self.fields[key] = value
        return self

class PySpider:
    def __init__(self, name, start_urls, allowed_domains=None):
        self.name = name
        self.start_urls = start_urls
        self.allowed_domains = allowed_domains or []

class EngineStats:
    def __init__(self):
        self.request_count = 1
        self.response_count = 1
        self.item_count = 1
        self.error_count = 0
        self.duration_seconds = 0.5
        self.requests_per_second = 2.0

class PyEngine:
    def __init__(self, spider):
        self.spider = spider
    
    def run(self):
        print(f"[MOCK] Running engine with spider: {self.spider.name}")
        # Simulate processing each start URL
        for url in self.spider.start_urls:
            response = PyResponse(url)
            print(f"[MOCK] Processing URL: {url}")
        return EngineStats()

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