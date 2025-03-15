"""
Scrapy-RS: High-performance web crawler written in Rust with Python bindings
"""

__version__ = "0.1.0"

try:
    from .scrapy_rs import *
except ImportError:
    print("Warning: Failed to import Rust extension. Using Python fallback.")
    
    def hello():
        """
        A simple function that returns a greeting.
        This is a fallback implementation when the Rust extension is not available.
        """
        return "Hello from Python fallback! (Rust extension not available)"