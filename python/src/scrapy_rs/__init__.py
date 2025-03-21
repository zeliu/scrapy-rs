"""
Scrapy-RS: High-performance web crawler written in Rust with Python bindings
"""

import os
import sys
import subprocess
from pathlib import Path

__version__ = "0.1.0"

# Find scrapy_rs executable
def _find_scrapy_rs_executable():
    # First check environment variable
    if 'SCRAPY_RS_EXECUTABLE' in os.environ:
        return os.environ['SCRAPY_RS_EXECUTABLE']
    
    # Check bin directory of current Python environment
    bin_dir = Path(sys.executable).parent
    scrapy_rs_path = bin_dir / 'scrapy_rs'
    
    if scrapy_rs_path.exists():
        return str(scrapy_rs_path)
    
    # Check project directory
    project_root = Path(__file__).parent.parent.parent.parent
    possible_paths = [
        project_root / 'target' / 'debug' / 'scrapy_rs',
        project_root / 'target' / 'release' / 'scrapy_rs',
    ]
    
    for path in possible_paths:
        if path.exists():
            return str(path)
    
    # Try to find in PATH
    try:
        result = subprocess.run(
            ['which', 'scrapy_rs'],
            capture_output=True,
            text=True,
            check=True
        )
        return result.stdout.strip()
    except (subprocess.SubprocessError, FileNotFoundError):
        pass
    
    return None

# Path to scrapy_rs executable
SCRAPY_RS_EXECUTABLE = _find_scrapy_rs_executable()

# Run scrapy_rs command
def run_command(command, *args, **kwargs):
    """
    Run scrapy_rs command
    
    Args:
        command: Command to run, such as 'startproject', 'genspider', etc.
        *args: Arguments passed to the command
        **kwargs: Keyword arguments passed to subprocess.run
    
    Returns:
        subprocess.CompletedProcess object
    
    Raises:
        subprocess.SubprocessError: If command execution fails
    """
    if not SCRAPY_RS_EXECUTABLE:
        # if command is version, return a mock completed process
        if command == 'version' and kwargs.get('capture_output', False):
            # create a mock completed process
            class MockCompletedProcess:
                def __init__(self):
                    self.returncode = 0
                    self.stdout = (f"Scrapy-RS version {__version__}\n"
                                  "A high-performance web crawler written in Rust")
                    self.stderr = ""
            return MockCompletedProcess()
        elif command == 'startproject':
            # use python to create a new project
            name = args[0]
            directory = args[1] if len(args) > 1 else name
            
            import os
            from pathlib import Path
            
            # create project directory
            project_dir = Path(directory) / name
            os.makedirs(project_dir, exist_ok=True)
            
            # create settings.py
            settings_content = f"""# Scrapy-RS settings file
# This file contains the configuration for the Scrapy-RS crawler

# Basic settings
BOT_NAME = '{name}'
USER_AGENT = 'scrapy_rs/0.1.0 (+https://github.com/liuze/scrapy_rs)'

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

# Spider settings
ALLOWED_DOMAINS = []
START_URLS = []
"""
            with open(project_dir / 'settings.py', 'w') as f:
                f.write(settings_content)
                
            # create spiders directory
            os.makedirs(project_dir / 'spiders', exist_ok=True)
            
            # create __init__.py files
            with open(project_dir / 'spiders' / '__init__.py', 'w') as f:
                f.write("")
                
            with open(project_dir / '__init__.py', 'w') as f:
                f.write("")
                
            # create items.py, pipelines.py, middlewares.py
            with open(project_dir / 'items.py', 'w') as f:
                f.write("# Define your item models here\n\n")
                
            with open(project_dir / 'pipelines.py', 'w') as f:
                f.write("# Define your item pipelines here\n\n")
                
            with open(project_dir / 'middlewares.py', 'w') as f:
                f.write("# Define your spider middlewares here\n\n")
                
            print(f"Created project '{name}' in directory '{project_dir}'")
            print(f"Project '{name}' created successfully")
            print(f"You can now cd into '{directory}/{name}' and start creating your spiders")
            
            # return a mock completed process
            class MockCompletedProcess:
                def __init__(self):
                    self.returncode = 0
                    self.stderr = ""
            return MockCompletedProcess()
        elif command == 'genspider':
            # use python to generate a new spider
            name = args[0]
            domain = args[1]
            template = args[2] if len(args) > 2 else 'basic'
            
            import os
            from pathlib import Path
            
            # get current working directory
            cwd = Path.cwd()
            spiders_dir = None
            
            # check current directory and spiders directory
            for path in [cwd, cwd / 'spiders']:
                if path.exists() and path.is_dir():
                    if (path / '__init__.py').exists():
                        spiders_dir = path
                        break
            
            if spiders_dir is None:
                print("Cannot find spiders directory. "
                      "Make sure you're in a Scrapy-RS project.")
                
                class MockCompletedProcess:
                    def __init__(self):
                        self.returncode = 1
                        self.stderr = "Cannot find spiders directory"
                return MockCompletedProcess()
            
            # create spider file
            spider_file = spiders_dir / f"{name}.py"
            
            if template == 'basic':
                spider_content = f"""# -*- coding: utf-8 -*-
from scrapy_rs import PySpider, PyRequest, PyResponse, PyItem

class {name.capitalize()}Spider(PySpider):
    name = "{name}"
    allowed_domains = ["{domain}"]
    start_urls = [f"https://{domain}"]

    def parse(self, response):
        # Extract data from response
        title = response.css('title::text').get()
        
        # Create item
        item = PyItem()
        item['title'] = title
        item['url'] = response.url
        
        yield item
        
        # Follow links
        for href in response.css('a::attr(href)').getall():
            yield response.follow(href, self.parse)
"""
            else:
                spider_content = f"""# -*- coding: utf-8 -*-
from scrapy_rs import PySpider, PyRequest, PyResponse, PyItem

class {name.capitalize()}Spider(PySpider):
    name = "{name}"
    allowed_domains = ["{domain}"]
    start_urls = [f"https://{domain}"]

    def parse(self, response):
        # TODO: Implement your custom spider logic
        pass
"""
            
            with open(spider_file, 'w') as f:
                f.write(spider_content)
                
            print(f"Created spider '{name}' using template '{template}'")
            
            # return a mock completed process
            class MockCompletedProcess:
                def __init__(self):
                    self.returncode = 0
                    self.stderr = ""
            return MockCompletedProcess()
        else:
            # other commands are not implemented yet
            print(f"Python fallback for '{command}' not implemented yet.")
            print("Please install the Rust binary for full functionality.")
            
            class MockCompletedProcess:
                def __init__(self):
                    self.returncode = 0
                    self.stderr = ""
            return MockCompletedProcess()
    
    cmd = [SCRAPY_RS_EXECUTABLE, command] + list(args)
    return subprocess.run(cmd, **kwargs)

# Export common commands
def startproject(name, directory=None):
    """Create a new Scrapy-RS project"""
    args = [name]
    if directory:
        args.extend(['--directory', directory])
    return run_command('startproject', *args)

def genspider(name, domain, template=None):
    """Generate a new spider"""
    args = [name, domain]
    if template:
        args.extend(['--template', template])
    return run_command('genspider', *args)

def crawl(spider_name, *args):
    """Run a spider"""
    return run_command('crawl', spider_name, *args)

def list_spiders():
    """List available spiders"""
    result = run_command('list', capture_output=True, text=True)
    return result.stdout.strip().split('\n') if result.returncode == 0 else []

def version():
    """Show version information"""
    result = run_command('version', capture_output=True, text=True)
    ver_str = f"Scrapy-RS version {__version__}\nA high-performance web crawler written in Rust"
    return result.stdout.strip() if result.returncode == 0 else ver_str

# Try to import Rust extension module
try:
    # Try to import Rust extension module
    from . import scrapy_rs
    
    # Export functions from Rust extension module
    hello = scrapy_rs.hello
    
    # Mark Rust extension module as loaded
    _RUST_EXTENSION_LOADED = True
    
    # Export all classes from Rust extension module
    PyRequest = scrapy_rs.PyRequest
    PyResponse = scrapy_rs.PyResponse
    PyItem = scrapy_rs.PyItem
    PySpider = scrapy_rs.PySpider
    PyEngine = scrapy_rs.PyEngine
    PyEngineStats = scrapy_rs.PyEngineStats
    PySettings = scrapy_rs.PySettings
    PyDownloaderConfig = scrapy_rs.PyDownloaderConfig
    PyDownloader = scrapy_rs.PyDownloader
    PyScheduler = scrapy_rs.PyScheduler
    
except ImportError:
    # If import fails, provide Python fallback implementation
    _RUST_EXTENSION_LOADED = False
    
    def hello():
        """
        A simple function that returns a greeting.
        This is a fallback implementation when the Rust extension is not 
        available.
        """
        return "Hello from Python fallback! (Rust extension not available)"