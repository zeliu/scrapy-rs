#!/usr/bin/env python3
"""
Advanced example of using RS-Spider from Python.
This example demonstrates more features of the RS-Spider, including:
- Custom request headers
- Response parsing with link extraction
- Item processing
- Error handling
"""

import sys
import os
import time
import re
import json

# Add parent directory to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
try:
    from src.rs_spider import PySpider, PyEngine, PyItem, PyRequest, PyResponse
except ImportError:
    print("Unable to import rs_spider module. Please make sure Python bindings are properly installed.")
    print("Current Python path:", sys.path)
    sys.exit(1)

class NewsSpider:
    """A spider that crawls a news website and extracts articles."""
    
    def __init__(self, name, start_urls, allowed_domains=None, max_pages=5):
        """Initialize the spider with a name, start URLs, and optional allowed domains."""
        self.spider = PySpider(name, start_urls, allowed_domains)
        self.engine = PyEngine(self.spider)
        self.max_pages = max_pages
        self.visited_urls = set()
        self.page_count = 0
        
    def parse(self, response):
        """Parse a response and extract data."""
        self.page_count += 1
        self.visited_urls.add(response.url)
        
        print(f"Parsing {response.url} (status: {response.status})")
        
        # Extract items from the response
        items = self.extract_items(response)
        
        # Extract links from the response and create new requests
        new_requests = []
        if self.page_count < self.max_pages:
            new_requests = self.extract_links(response)
            
        return items, new_requests
    
    def extract_items(self, response):
        """Extract items from the response."""
        # In a real spider, you would use a library like BeautifulSoup to parse the HTML
        # For this example, we'll just create a dummy item
        
        item = PyItem("article")
        item.set("url", response.url)
        item.set("title", f"Article from {response.url}")
        item.set("content", "This is the content of the article.")
        item.set("timestamp", time.time())
        
        return [item]
    
    def extract_links(self, response):
        """Extract links from the response and create new requests."""
        # In a real spider, you would use a library like BeautifulSoup to parse the HTML
        # For this example, we'll just create some dummy links
        
        # Simulate finding links in the HTML
        dummy_links = [
            f"{response.url}/page1",
            f"{response.url}/page2",
            f"{response.url}/page3",
        ]
        
        requests = []
        for link in dummy_links:
            if link not in self.visited_urls:
                request = PyRequest(link)
                # Add custom headers
                request.set_meta("referrer", response.url)
                request.set_meta("depth", self.page_count)
                requests.append(request)
                
        return requests
    
    def run(self):
        """Run the spider and print the results."""
        print(f"Starting spider: {self.spider.name}")
        print(f"Start URLs: {self.spider.start_urls}")
        print(f"Allowed domains: {self.spider.allowed_domains}")
        print(f"Max pages: {self.max_pages}")
        
        # Run the engine
        try:
            stats = self.engine.run()
            
            # Print the results
            print("\nCrawl completed!")
            print(f"Requests: {stats.request_count}")
            print(f"Responses: {stats.response_count}")
            print(f"Items: {stats.item_count}")
            print(f"Errors: {stats.error_count}")
            print(f"Duration: {stats.duration_seconds:.2f} seconds")
            print(f"Requests per second: {stats.requests_per_second:.2f}")
            
            # Print visited URLs
            print("\nVisited URLs:")
            for url in self.visited_urls:
                print(f"- {url}")
                
        except Exception as e:
            print(f"Error running spider: {e}")


class QuotesSpider:
    """A spider that crawls quotes.toscrape.com and extracts quotes."""
    
    def __init__(self):
        """Initialize the spider."""
        self.spider = PySpider(
            "quotes",
            ["http://quotes.toscrape.com"],
            ["quotes.toscrape.com"]
        )
        self.engine = PyEngine(self.spider)
        self.visited_urls = set()
        
    def parse(self, response):
        """Parse a response and extract quotes."""
        self.visited_urls.add(response.url)
        
        print(f"Parsing {response.url} (status: {response.status})")
        
        # In a real spider, you would use a library like BeautifulSoup to parse the HTML
        # For this example, we'll just create some dummy quotes
        
        items = []
        for i in range(1, 6):
            quote = PyItem("quote")
            quote.set("text", f"Quote {i} from {response.url}")
            quote.set("author", f"Author {i}")
            quote.set("tags", ["tag1", "tag2", "tag3"])
            items.append(quote)
        
        # Extract the "next" link if it exists
        new_requests = []
        if "page/1" not in response.url:
            next_url = "http://quotes.toscrape.com/page/1/"
            if next_url not in self.visited_urls:
                request = PyRequest(next_url)
                new_requests.append(request)
                
        return items, new_requests
    
    def run(self):
        """Run the spider and print the results."""
        print(f"Starting QuotesSpider")
        
        # Run the engine
        try:
            stats = self.engine.run()
            
            # Print the results
            print("\nCrawl completed!")
            print(f"Requests: {stats.request_count}")
            print(f"Responses: {stats.response_count}")
            print(f"Items: {stats.item_count}")
            print(f"Errors: {stats.error_count}")
            print(f"Duration: {stats.duration_seconds:.2f} seconds")
            print(f"Requests per second: {stats.requests_per_second:.2f}")
            
        except Exception as e:
            print(f"Error running spider: {e}")


def main():
    """Run the example spiders."""
    if len(sys.argv) > 1 and sys.argv[1] == "quotes":
        # Run the quotes spider
        spider = QuotesSpider()
    else:
        # Run the news spider
        spider = NewsSpider(
            name="news",
            start_urls=["https://example.com/news"],
            allowed_domains=["example.com"],
            max_pages=3
        )
    
    # Run the spider
    spider.run()


if __name__ == "__main__":
    main() 