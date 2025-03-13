# RS-Spider settings file
# This file contains the configuration for the RS-Spider crawler

# Basic settings
BOT_NAME = 'rs_spider'
USER_AGENT = 'rs-spider/0.1.0 (+https://github.com/zeliu/rs_spider)'

# Crawl settings
CONCURRENT_REQUESTS = 16
DOWNLOAD_DELAY_MS = 0
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
LOG_LEVEL = 'INFO'
LOG_REQUESTS = True
LOG_ITEMS = True
LOG_STATS = True
STATS_INTERVAL_SECS = 60

# Pipelines
ITEM_PIPELINES = [
    'LogPipeline',
    'JsonFilePipeline',
]

# Pipeline settings
JSON_FILE_PATH = 'items.json'

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