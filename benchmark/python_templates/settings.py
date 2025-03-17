BOT_NAME = 'benchmark'

SPIDER_MODULES = ['benchmark.spiders']
NEWSPIDER_MODULE = 'benchmark.spiders'

# Obey robots.txt rules
ROBOTSTXT_OBEY = True

# Configure maximum concurrent requests
CONCURRENT_REQUESTS = 16

# Configure a delay for requests for the same website
DOWNLOAD_DELAY = 0.0

# The download handlers
DOWNLOAD_HANDLERS = {}

# Set the user agent
USER_AGENT = 'scrapy-benchmark/1.0'

# Configure item pipelines
ITEM_PIPELINES = {
    'benchmark.pipelines.BenchmarkPipeline': 300,
}

# Enable or disable redirect follows
REDIRECT_ENABLED = True

# 其他设置会在运行时被添加 