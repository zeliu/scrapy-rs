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
        result = subprocess.run(['which', 'scrapy_rs'], 
                               capture_output=True, 
                               text=True, 
                               check=True)
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
        FileNotFoundError: If scrapy_rs executable is not found
        subprocess.SubprocessError: If command execution fails
    """
    if not SCRAPY_RS_EXECUTABLE:
        # If executable is not found but command needs output, return simulated output
        if command == 'version' and kwargs.get('capture_output', False):
            # Create a mock CompletedProcess object
            class MockCompletedProcess:
                def __init__(self):
                    self.returncode = 0
                    self.stdout = f"Scrapy-RS version {__version__}\nA high-performance web crawler written in Rust"
                    self.stderr = ""
            return MockCompletedProcess()
        else:
            # For other commands, still raise exception
            raise FileNotFoundError("Could not find scrapy_rs executable")
    
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
    return result.stdout.strip() if result.returncode == 0 else f"Scrapy-RS version {__version__}\nA high-performance web crawler written in Rust"

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
        This is a fallback implementation when the Rust extension is not available.
        """
        return "Hello from Python fallback! (Rust extension not available)"