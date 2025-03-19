import scrapy
import json
import time
from urllib.parse import urljoin

class BenchmarkItem(scrapy.Item):
    url = scrapy.Field()
    title = scrapy.Field()
    links = scrapy.Field()
    timestamp = scrapy.Field()

class BenchmarkSpider(scrapy.Spider):
    name = 'benchmark'
    custom_settings = {
        'ITEM_PIPELINES': {
            'benchmark.pipelines.BenchmarkPipeline': 300,
        },
    }

    def __init__(self, *args, **kwargs):
        super(BenchmarkSpider, self).__init__(*args, **kwargs)
        self.start_urls = []  # This will be replaced with actual URLs at runtime
        self.crawled_count = 0
        self.max_pages = 100  # This will be replaced with actual page limit at runtime
        self.max_depth = 2    # This will be replaced with actual depth limit at runtime

    def parse(self, response):
        # Check if we've reached the page limit
        self.crawled_count += 1
        if self.crawled_count > self.max_pages:
            return

        # Extract the title
        title = response.css('title::text').get()

        # Extract links
        links = []
        for href in response.css('a::attr(href)'):
            url = href.get()
            if url and not url.startswith('#') and not url.startswith('javascript:'):
                full_url = response.urljoin(url)
                links.append(full_url)

                # Follow the link if we haven't reached the max depth
                depth = response.meta.get('depth', 1)
                if depth < self.max_depth:
                    yield scrapy.Request(full_url, callback=self.parse, meta={'depth': depth + 1})

        # Create an item
        item = BenchmarkItem()
        item['url'] = response.url
        item['title'] = title
        item['links'] = links
        item['timestamp'] = time.time()
        yield item 