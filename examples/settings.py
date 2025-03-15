# Scrapy-RS settings file
# This file contains the configuration for the Scrapy-RS crawler

# Bot name and user agent
BOT_NAME = 'scrapy_rs'
USER_AGENT = 'scrapy-rs/0.1.0 (+https://github.com/zeliu/scrapy_rs)'

# Crawl responsibly by identifying yourself on the user agent
# USER_AGENT = 'Mozilla/5.0 (compatible; Scrapy-RS/0.1.0; +https://github.com/zeliu/scrapy_rs)'

# Configure maximum concurrent requests
CONCURRENT_REQUESTS = 16
CONCURRENT_REQUESTS_PER_DOMAIN = 8
CONCURRENT_REQUESTS_PER_IP = 0

# Configure a delay for requests for the same website (default: 0)
DOWNLOAD_DELAY = 0
# The download delay setting will honor only one of:
# CONCURRENT_REQUESTS_PER_DOMAIN = 16
# CONCURRENT_REQUESTS_PER_IP = 16

# Disable cookies (enabled by default)
COOKIES_ENABLED = True

# Disable Telnet Console (enabled by default)
TELNETCONSOLE_ENABLED = False

# Override the default request headers
DEFAULT_REQUEST_HEADERS = {
    'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
    'Accept-Language': 'en',
}

# Enable or disable spider middlewares
SPIDER_MIDDLEWARES = {
    # 'scrapy_rs.middlewares.MyCustomSpiderMiddleware': 543,
}

# Enable or disable downloader middlewares
DOWNLOADER_MIDDLEWARES = {
    # 'scrapy_rs.middlewares.MyCustomDownloaderMiddleware': 543,
}

# Configure item pipelines
ITEM_PIPELINES = {
    # 'scrapy_rs.pipelines.JsonFilePipeline': 300,
}

# Enable and configure the AutoThrottle extension
AUTOTHROTTLE_ENABLED = True
# The initial download delay
AUTOTHROTTLE_START_DELAY = 5
# The maximum download delay to be set in case of high latencies
AUTOTHROTTLE_MAX_DELAY = 60
# The average number of requests Scrapy should be sending in parallel to
# each remote server
AUTOTHROTTLE_TARGET_CONCURRENCY = 1.0
# Enable showing throttling stats for every response received:
AUTOTHROTTLE_DEBUG = False

# Enable and configure HTTP caching
HTTPCACHE_ENABLED = False
HTTPCACHE_EXPIRATION_SECS = 0
HTTPCACHE_DIR = 'httpcache'
HTTPCACHE_IGNORE_HTTP_CODES = []
HTTPCACHE_STORAGE = 'scrapy_rs.extensions.FilesystemCacheStorage'

# Configure logging
LOG_LEVEL = 'INFO'
LOG_FILE = None

# Crawl settings
REQUEST_TIMEOUT = 30
FOLLOW_REDIRECTS = True
MAX_RETRIES = 3
RETRY_ENABLED = True
RESPECT_ROBOTS_TXT = True

# Limits
MAX_DEPTH = None
MAX_REQUESTS_PER_DOMAIN = None
MAX_REQUESTS_PER_SPIDER = None

# Logging
LOG_REQUESTS = True
LOG_ITEMS = True
LOG_STATS = True
STATS_INTERVAL_SECS = 60

# Middleware
REQUEST_MIDDLEWARES = [
    'DefaultHeadersMiddleware',
    'RandomDelayMiddleware',
]

RESPONSE_MIDDLEWARES = [
    'ResponseLoggerMiddleware',
    'RetryMiddleware',
]

# Middleware settings
DEFAULT_HEADERS = {
    'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
    'Accept-Language': 'en',
}

# Spider settings
ALLOWED_DOMAINS = ['example.com']
START_URLS = ['https://example.com'] 