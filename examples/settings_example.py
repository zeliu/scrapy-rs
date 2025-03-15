#!/usr/bin/env python3
"""
Example of using settings in Scrapy-RS from Python.
This example demonstrates how to configure the crawler using settings.
"""

import sys
import os
import time

# Add parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
try:
    from scrapy_rs import PySpider, PyEngine, PySettings
except ImportError:
    print("Unable to import scrapy_rs module. Please make sure Python bindings are properly installed.")
    print("Current Python path:", sys.path)
    sys.exit(1)

def main():
    """Run the example."""
    print("Using settings for Scrapy-RS from Python")
    
    # Create settings
    settings = PySettings()
    settings.set("BOT_NAME", "example_bot")
    settings.set("USER_AGENT", "Scrapy-RS Example Bot/1.0")
    settings.set("CONCURRENT_REQUESTS", 4)
    settings.set("DOWNLOAD_DELAY", 1.0)
    settings.set("LOG_LEVEL", "INFO")
    
    # Create a spider
    spider = PySpider(
        name="example",
        start_urls=["https://example.com"],
        allowed_domains=["example.com"]
    )
    
    # Create an engine with settings
    engine = PyEngine(spider, settings)
    
    # Run the engine
    stats = engine.run()
    
    # Print the results
    print("\nCrawl completed!")
    print(f"Requests: {stats.request_count}")
    print(f"Responses: {stats.response_count}")
    print(f"Items: {stats.item_count}")
    print(f"Errors: {stats.error_count}")
    print(f"Duration: {stats.duration_seconds:.2f} seconds")
    print(f"Requests per second: {stats.requests_per_second:.2f}")

if __name__ == "__main__":
    main() 