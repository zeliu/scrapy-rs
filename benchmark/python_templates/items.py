import scrapy

class BenchmarkItem(scrapy.Item):
    """
    Item class for benchmark results.
    """
    url = scrapy.Field()
    title = scrapy.Field()
    links = scrapy.Field()
    timestamp = scrapy.Field() 