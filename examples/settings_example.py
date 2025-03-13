#!/usr/bin/env python3
"""
Example of using settings in RS-Spider from Python.
This example demonstrates how to use settings with the Python bindings.
"""

import sys
import os
import time

# Add site-packages directory to sys.path
site_packages = '/Users/liuze/Library/Python/3.9/lib/python3.9/site-packages'
if site_packages not in sys.path:
    sys.path.append(site_packages)

try:
    from rs_spider import PySpider, PyEngine, PySettings
except ImportError as e:
    print("Unable to import rs_spider module. Please make sure Python bindings are properly installed.")
    print(f"Import error: {e}")
    print("Current Python path:", sys.path)
    sys.exit(1)

def main():
    """Run the example."""
    print("Using settings for RS-Spider from Python")
    
    # Load settings from file
    print("Loading settings from file...")
    settings_path = os.path.abspath(os.path.join(os.path.dirname(__file__), '../../examples/settings.py'))
    settings = PySettings.from_file(settings_path)
    
    # Get specific settings
    bot_name = settings.get("BOT_NAME")
    user_agent = settings.get("USER_AGENT")
    concurrent_requests = settings.get("CONCURRENT_REQUESTS")
    download_delay_ms = settings.get("DOWNLOAD_DELAY_MS")
    follow_redirects = settings.get("FOLLOW_REDIRECTS")
    start_urls = settings.get("START_URLS")
    allowed_domains = settings.get("ALLOWED_DOMAINS")
    
    print("\nSettings:")
    print(f"  Bot name: {bot_name}")
    print(f"  User agent: {user_agent}")
    print(f"  Concurrent requests: {concurrent_requests}")
    print(f"  Download delay (ms): {download_delay_ms}")
    print(f"  Follow redirects: {follow_redirects}")
    print(f"  Start URLs: {start_urls}")
    print(f"  Allowed domains: {allowed_domains}")
    
    # Create a spider
    spider = PySpider("example", start_urls, allowed_domains)
    
    # Create an engine
    engine = PyEngine(spider)
    
    print("\nEngine created with settings from file.")
    print("In a real application, you would now run the engine with engine.run()")
    
    # In a real application, you would run the engine:
    # stats = engine.run()
    # print("Crawl completed!")
    # print(f"Requests: {stats.request_count}")
    # print(f"Responses: {stats.response_count}")
    # print(f"Items: {stats.item_count}")
    # print(f"Errors: {stats.error_count}")
    # print(f"Duration: {stats.duration_seconds:.2f} seconds")
    # print(f"Requests per second: {stats.requests_per_second:.2f}")


if __name__ == "__main__":
    main() 