#!/usr/bin/env python3
"""
Complex Spider Example for Scrapy-RS

This example demonstrates a more complex spider that:
1. Crawls a book catalog website (books.toscrape.com)
2. Extracts book listings from category pages
3. Follows links to detail pages to extract more information
4. Uses custom middleware for request and response processing
5. Implements custom item pipeline for data processing
6. Handles pagination

This example is designed to test compatibility between Scrapy and Scrapy-RS.
"""

import sys
import os
import time
import re
import json
import logging
from urllib.parse import urljoin
from bs4 import BeautifulSoup

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(message)s',
    handlers=[logging.StreamHandler()]
)
logger = logging.getLogger(__name__)

try:
    # Import scrapy_rs modules
    import scrapy_rs
    from scrapy_rs import PyRequest, PyResponse, PyItem, PySpider, PyEngine, PySettings
    
    # Print the version to confirm we're using scrapy-rs
    logger.info(f"Using Scrapy-RS version: {scrapy_rs.__version__}")
    USING_SCRAPY_RS = True
    
except ImportError as e:
    logger.warning(f"Failed to import scrapy_rs: {e}")
    logger.warning("Falling back to mock implementation")
    USING_SCRAPY_RS = False
    
    # Mock classes to simulate the Rust bindings
    class PyRequest:
        def __init__(self, url, method="GET", headers=None, body=None, cookies=None, 
                     meta=None, dont_filter=False, timeout=None, encoding=None, 
                     flags=None, proxy=None):
            self.url = url
            self.method = method
            self.headers = headers or {}
            self.body = body
            self.cookies = cookies or {}
            self.meta = meta or {}
            self.dont_filter = dont_filter
            self.timeout = timeout
            self.encoding = encoding
            self.flags = flags or []
            self.proxy = proxy
        
        def set_meta(self, key, value):
            self.meta[key] = value
            return self
        
        def set_cookie(self, key, value):
            self.cookies[key] = value
            return self
        
        def set_dont_filter(self, value):
            self.dont_filter = value
            return self
        
        def set_timeout(self, value):
            self.timeout = value
            return self
        
        def set_encoding(self, value):
            self.encoding = value
            return self
        
        def add_flag(self, flag):
            if flag not in self.flags:
                self.flags.append(flag)
            return self
        
        def has_flag(self, flag):
            return flag in self.flags
        
        def set_proxy(self, proxy):
            self.proxy = proxy
            return self
        
        def __str__(self):
            return f"PyRequest({self.url})"

    class PyResponse:
        def __init__(self, url, status=200, headers=None, body=None, request=None, 
                     flags=None, certificate=None, ip_address=None, protocol=None):
            self.url = url
            self.status = status
            self.headers = headers or {}
            self.body = body or b""
            self.request = request
            self.flags = flags or []
            self.certificate = certificate
            self.ip_address = ip_address
            self.protocol = protocol
        
        def text(self):
            return self.body.decode('utf-8', errors='replace')
        
        def add_flag(self, flag):
            if flag not in self.flags:
                self.flags.append(flag)
            return self
        
        def has_flag(self, flag):
            return flag in self.flags
        
        def urljoin(self, url):
            return urljoin(self.url, url)
        
        def follow(self, url, **kwargs):
            absolute_url = self.urljoin(url)
            return PyRequest(absolute_url, **kwargs)
        
        def follow_all(self, urls, **kwargs):
            return [self.follow(url, **kwargs) for url in urls]
        
        def copy(self):
            return PyResponse(
                url=self.url,
                status=self.status,
                headers=self.headers.copy(),
                body=self.body,
                request=self.request,
                flags=self.flags.copy(),
                certificate=self.certificate,
                ip_address=self.ip_address,
                protocol=self.protocol
            )
        
        def replace(self, **kwargs):
            new_response = self.copy()
            for key, value in kwargs.items():
                setattr(new_response, key, value)
            return new_response
        
        def __str__(self):
            return f"PyResponse({self.url}, status={self.status})"

    class PyItem:
        def __init__(self, item_type):
            self.type = item_type
            self.fields = {}
        
        def set(self, key, value):
            self.fields[key] = value
            return self
        
        def get(self, key, default=None):
            return self.fields.get(key, default)
        
        def __str__(self):
            return f"PyItem({self.type}, {len(self.fields)} fields)"

    class PySpider:
        def __init__(self, name, start_urls, allowed_domains=None):
            self.name = name
            self.start_urls = start_urls
            self.allowed_domains = allowed_domains or []

    class PyEngine:
        def __init__(self, spider):
            self.spider = spider
        
        def run(self):
            logger.info(f"[MOCK] Running engine with spider: {self.spider.name}")
            return MockEngineStats()

    class MockEngineStats:
        def __init__(self):
            self.request_count = 10
            self.response_count = 10
            self.item_count = 20
            self.error_count = 0
            self.duration_seconds = 2.5
            self.requests_per_second = 4.0

    class PySettings:
        def __init__(self):
            self.settings = {}
        
        def set(self, key, value):
            self.settings[key] = value
            return self
        
        def get(self, key, default=None):
            return self.settings.get(key, default)


# Custom middleware classes
class UserAgentMiddleware:
    """Middleware to set a custom User-Agent header on outgoing requests."""
    
    def __init__(self, user_agent="Scrapy-RS/1.0"):
        self.user_agent = user_agent
        logger.info(f"Initialized UserAgentMiddleware with User-Agent: {user_agent}")
    
    def process_request(self, request):
        """Add User-Agent header to the request."""
        request.headers["User-Agent"] = self.user_agent
        request.add_flag("user-agent-added")
        logger.debug(f"Added User-Agent to request: {request.url}")
        return request


class ResponseLogMiddleware:
    """Middleware to log response information."""
    
    def process_response(self, response):
        """Log information about the response."""
        logger.info(f"Received response: {response.url} (status: {response.status})")
        response.add_flag("logged")
        return response


class DelayMiddleware:
    """Middleware to add a delay between requests."""
    
    def __init__(self, delay_seconds=1.0):
        self.delay_seconds = delay_seconds
        logger.info(f"Initialized DelayMiddleware with delay: {delay_seconds}s")
    
    def process_request(self, request):
        """Add a delay before processing the request."""
        logger.debug(f"Delaying request for {self.delay_seconds}s: {request.url}")
        time.sleep(self.delay_seconds)
        request.add_flag("delayed")
        return request


class BooksPipeline:
    """Pipeline to process book items."""
    
    def __init__(self, output_file=None):
        self.items = []
        self.output_file = output_file
        logger.info(f"Initialized BooksPipeline with output file: {output_file}")
    
    def process_item(self, item):
        """Process a book item."""
        if item.type == "book":
            # Add a timestamp
            item.set("processed_at", time.time())
            
            # Store the item
            self.items.append(item)
            
            logger.info(f"Processed book: {item.get('title')} (Total: {len(self.items)})")
        
        return item
    
    def close(self):
        """Close the pipeline and save items if an output file is specified."""
        if self.output_file and self.items:
            try:
                # Convert items to a list of dictionaries
                items_data = [item.fields for item in self.items]
                
                # Save to JSON file
                with open(self.output_file, 'w') as f:
                    json.dump(items_data, f, indent=2)
                
                logger.info(f"Saved {len(self.items)} items to {self.output_file}")
            except Exception as e:
                logger.error(f"Failed to save items to {self.output_file}: {e}")


class BooksCatalogSpider:
    """A spider that crawls books.toscrape.com and extracts book information."""
    
    def __init__(self, max_pages=3, output_file=None):
        """Initialize the spider."""
        self.name = "books"
        self.start_urls = ["http://books.toscrape.com/"]
        self.allowed_domains = ["books.toscrape.com"]
        self.max_pages = max_pages
        self.visited_urls = set()
        self.page_count = 0
        
        # Create the spider object
        self.spider = PySpider(
            self.name,
            self.start_urls,
            self.allowed_domains
        )
        
        # Create settings
        self.settings = PySettings()
        self.settings.set("CONCURRENT_REQUESTS", 2)
        self.settings.set("DOWNLOAD_TIMEOUT", 30)
        
        # Create middleware
        self.user_agent_middleware = UserAgentMiddleware("BooksCatalogSpider/1.0")
        self.response_log_middleware = ResponseLogMiddleware()
        self.delay_middleware = DelayMiddleware(0.5)
        
        # Create pipeline
        self.pipeline = BooksPipeline(output_file)
        
        # Create engine
        self.engine = PyEngine(self.spider)
    
    def parse(self, response):
        """Parse the main page or category page."""
        self.page_count += 1
        self.visited_urls.add(response.url)
        
        logger.info(f"Parsing listing page: {response.url} (status: {response.status})")
        
        # Use BeautifulSoup to parse the HTML
        soup = BeautifulSoup(response.text(), 'html.parser')
        
        # Extract book links
        book_links = []
        for book_element in soup.select('article.product_pod h3 a'):
            book_url = book_element.get('href')
            if book_url:
                book_links.append(book_url)
        
        # Create requests for book detail pages
        book_requests = []
        for link in book_links[:5]:  # Limit to 5 books per page for the example
            absolute_url = response.urljoin(link)
            if absolute_url not in self.visited_urls:
                request = PyRequest(absolute_url)
                request.set_meta("page_type", "detail")
                book_requests.append(request)
        
        # Extract next page link if we haven't reached the maximum
        next_requests = []
        if self.page_count < self.max_pages:
            next_link = soup.select_one('li.next a')
            if next_link and next_link.get('href'):
                next_url = response.urljoin(next_link.get('href'))
                if next_url not in self.visited_urls:
                    request = PyRequest(next_url)
                    request.set_meta("page_type", "listing")
                    next_requests.append(request)
        
        # No items from listing pages, just requests for detail pages and next page
        return [], book_requests + next_requests
    
    def parse_book(self, response):
        """Parse a book detail page."""
        self.visited_urls.add(response.url)
        
        logger.info(f"Parsing book detail page: {response.url}")
        
        # Use BeautifulSoup to parse the HTML
        soup = BeautifulSoup(response.text(), 'html.parser')
        
        # Extract book information
        title = soup.select_one('div.product_main h1')
        price = soup.select_one('p.price_color')
        availability = soup.select_one('p.availability')
        description = soup.select_one('div#product_description + p')
        category = soup.select_one('ul.breadcrumb li:nth-child(3) a')
        rating = soup.select_one('p.star-rating')
        
        # Create a book item
        book = PyItem("book")
        book.set("url", response.url)
        book.set("title", title.text.strip() if title else "Unknown Title")
        book.set("price", price.text.strip() if price else "Unknown Price")
        book.set("availability", availability.text.strip() if availability else "Unknown")
        book.set("description", description.text.strip() if description else "No description available")
        book.set("category", category.text.strip() if category else "Unknown Category")
        
        if rating:
            # Extract rating class (e.g., "star-rating Three" -> "Three")
            rating_class = rating.get('class', [])
            rating_value = rating_class[1] if len(rating_class) > 1 else "Unknown"
            book.set("rating", rating_value)
        else:
            book.set("rating", "Unknown")
        
        # Extract product information table
        product_info = {}
        info_table = soup.select_one('table.table-striped')
        if info_table:
            for row in info_table.select('tr'):
                header = row.select_one('th')
                value = row.select_one('td')
                if header and value:
                    product_info[header.text.strip()] = value.text.strip()
        
        book.set("product_info", product_info)
        book.set("crawled_at", time.time())
        
        # No further requests from detail pages
        return [book], []
    
    def process_request(self, request):
        """Apply request middleware."""
        # Apply middleware in order
        request = self.user_agent_middleware.process_request(request)
        request = self.delay_middleware.process_request(request)
        return request
    
    def process_response(self, response):
        """Apply response middleware."""
        # Apply middleware in order
        response = self.response_log_middleware.process_response(response)
        return response
    
    def process_item(self, item):
        """Apply item pipeline."""
        return self.pipeline.process_item(item)
    
    def run(self):
        """Run the spider."""
        logger.info(f"Starting BooksCatalogSpider")
        logger.info(f"Max pages: {self.max_pages}")
        
        if USING_SCRAPY_RS:
            # Use the actual Scrapy-RS engine
            try:
                stats = self.engine.run()
                
                # Print the results
                logger.info("\nCrawl completed!")
                logger.info(f"Requests: {stats.request_count}")
                logger.info(f"Responses: {stats.response_count}")
                logger.info(f"Items: {stats.item_count}")
                logger.info(f"Errors: {stats.error_count}")
                logger.info(f"Duration: {stats.duration_seconds:.2f} seconds")
                logger.info(f"Requests per second: {stats.requests_per_second:.2f}")
            except Exception as e:
                logger.error(f"Error running spider with Scrapy-RS: {e}")
        else:
            # Mock implementation for testing without Scrapy-RS
            logger.info("Using mock implementation")
            self._run_mock()
        
        # Close the pipeline
        self.pipeline.close()
    
    def _run_mock(self):
        """Run a mock crawl for testing without Scrapy-RS."""
        try:
            # Process start URLs
            for start_url in self.start_urls:
                # Create a request
                request = PyRequest(start_url)
                request.set_meta("page_type", "listing")
                
                # Process the request with middleware
                request = self.process_request(request)
                
                # Create a mock response
                response = PyResponse(
                    url=request.url,
                    status=200,
                    headers={"Content-Type": "text/html"},
                    body=self._get_mock_html(request.url),
                    request=request
                )
                
                # Process the response with middleware
                response = self.process_response(response)
                
                # Parse the response
                if request.meta.get("page_type") == "detail":
                    items, requests = self.parse_book(response)
                else:
                    items, requests = self.parse(response)
                
                # Process items
                for item in items:
                    self.process_item(item)
                
                # Process new requests (limited depth for the mock)
                if self.page_count < self.max_pages:
                    for new_request in requests[:3]:  # Limit to 3 requests for the mock
                        # Process the request with middleware
                        new_request = self.process_request(new_request)
                        
                        # Create a mock response
                        new_response = PyResponse(
                            url=new_request.url,
                            status=200,
                            headers={"Content-Type": "text/html"},
                            body=self._get_mock_html(new_request.url),
                            request=new_request
                        )
                        
                        # Process the response with middleware
                        new_response = self.process_response(new_response)
                        
                        # Parse the response
                        if new_request.meta.get("page_type") == "detail":
                            new_items, _ = self.parse_book(new_response)
                        else:
                            new_items, _ = self.parse(new_response)
                        
                        # Process items
                        for new_item in new_items:
                            self.process_item(new_item)
            
            # Print mock results
            logger.info("\nMock crawl completed!")
            logger.info(f"Pages visited: {len(self.visited_urls)}")
            logger.info(f"Books extracted: {len(self.pipeline.items)}")
            
        except Exception as e:
            logger.error(f"Error in mock crawl: {e}")
    
    def _get_mock_html(self, url):
        """Generate mock HTML for testing."""
        if "index.html" in url or url.endswith(".com/"):
            # Main page
            return b"""
            <html>
                <body>
                    <div class="page_inner">
                        <ul class="breadcrumb">
                            <li><a href="index.html">Home</a></li>
                        </ul>
                        <div>
                            <section>
                                <div>
                                    <ol class="row">
                                        <li class="col-xs-6 col-sm-4 col-md-3 col-lg-3">
                                            <article class="product_pod">
                                                <h3><a href="catalogue/a-light-in-the-attic_1000/index.html">A Light in the Attic</a></h3>
                                                <div class="product_price">
                                                    <p class="price_color">GBP51.77</p>
                                                </div>
                                            </article>
                                        </li>
                                        <li class="col-xs-6 col-sm-4 col-md-3 col-lg-3">
                                            <article class="product_pod">
                                                <h3><a href="catalogue/tipping-the-velvet_999/index.html">Tipping the Velvet</a></h3>
                                                <div class="product_price">
                                                    <p class="price_color">GBP53.74</p>
                                                </div>
                                            </article>
                                        </li>
                                    </ol>
                                    <div>
                                        <ul class="pager">
                                            <li class="next"><a href="catalogue/page-2.html">next</a></li>
                                        </ul>
                                    </div>
                                </div>
                            </section>
                        </div>
                    </div>
                </body>
            </html>
            """
        elif "page-" in url:
            # Pagination page
            page_num = int(re.search(r"page-(\d+)", url).group(1))
            next_page = f'<li class="next"><a href="catalogue/page-{page_num + 1}.html">next</a></li>' if page_num < 5 else ""
            return f"""
            <html>
                <body>
                    <div class="page_inner">
                        <ul class="breadcrumb">
                            <li><a href="../index.html">Home</a></li>
                            <li>Page {page_num}</li>
                        </ul>
                        <div>
                            <section>
                                <div>
                                    <ol class="row">
                                        <li class="col-xs-6 col-sm-4 col-md-3 col-lg-3">
                                            <article class="product_pod">
                                                <h3><a href="../catalogue/book-{page_num}-1.html">Book {page_num}-1</a></h3>
                                                <div class="product_price">
                                                    <p class="price_color">GBP{20 + page_num}.99</p>
                                                </div>
                                            </article>
                                        </li>
                                        <li class="col-xs-6 col-sm-4 col-md-3 col-lg-3">
                                            <article class="product_pod">
                                                <h3><a href="../catalogue/book-{page_num}-2.html">Book {page_num}-2</a></h3>
                                                <div class="product_price">
                                                    <p class="price_color">GBP{30 + page_num}.50</p>
                                                </div>
                                            </article>
                                        </li>
                                    </ol>
                                    <div>
                                        <ul class="pager">
                                            <li class="previous"><a href="catalogue/page-{page_num - 1}.html">previous</a></li>
                                            {next_page}
                                        </ul>
                                    </div>
                                </div>
                            </section>
                        </div>
                    </div>
                </body>
            </html>
            """.encode('utf-8')
        else:
            # Book detail page
            book_id = url.split('/')[-2] if '/' in url else "unknown"
            return f"""
            <html>
                <body>
                    <div class="page_inner">
                        <ul class="breadcrumb">
                            <li><a href="../../index.html">Home</a></li>
                            <li><a href="../category/books_1/index.html">Books</a></li>
                            <li><a href="../category/books/fiction_10/index.html">Fiction</a></li>
                            <li class="active">Book {book_id}</li>
                        </ul>
                        <div class="row">
                            <div class="col-sm-6">
                                <div class="product_main">
                                    <h1>Book Title for {book_id}</h1>
                                    <p class="price_color">GBP24.99</p>
                                    <p class="availability">In stock (14 available)</p>
                                    <p class="star-rating Four"></p>
                                </div>
                            </div>
                        </div>
                        <div id="product_description" class="sub-header">
                            <h2>Product Description</h2>
                        </div>
                        <p>This is a sample book description for {book_id}. It contains information about the book.</p>
                        <table class="table table-striped">
                            <tr>
                                <th>UPC</th>
                                <td>12345{book_id}</td>
                            </tr>
                            <tr>
                                <th>Product Type</th>
                                <td>Books</td>
                            </tr>
                            <tr>
                                <th>Price (excl. tax)</th>
                                <td>GBP20.00</td>
                            </tr>
                            <tr>
                                <th>Price (incl. tax)</th>
                                <td>GBP24.99</td>
                            </tr>
                            <tr>
                                <th>Availability</th>
                                <td>In stock</td>
                            </tr>
                            <tr>
                                <th>Number of reviews</th>
                                <td>5</td>
                            </tr>
                        </table>
                    </div>
                </body>
            </html>
            """.encode('utf-8')


def main():
    """Run the example spider."""
    # Parse command line arguments
    import argparse
    parser = argparse.ArgumentParser(description='Run the BooksCatalogSpider')
    parser.add_argument('--max-pages', type=int, default=3, help='Maximum number of pages to crawl')
    parser.add_argument('--output', type=str, default='books.json', help='Output file for scraped data')
    args = parser.parse_args()
    
    # Create and run the spider
    spider = BooksCatalogSpider(max_pages=args.max_pages, output_file=args.output)
    spider.run()


if __name__ == "__main__":
    main() 