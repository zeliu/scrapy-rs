#!/usr/bin/env python3
"""
Simple example script to demonstrate how to use the Scrapy-RS Python bindings
"""

import scrapy_rs

def main():
    # Print the version
    print("Scrapy-RS version:", scrapy_rs.__version__)
    
    # Use the hello function
    greeting = scrapy_rs.hello()
    print(greeting)
    
    # If the Rust extension is available, you can use more advanced features
    # For example, if the PyRequest class is available:
    try:
        from scrapy_rs import PyRequest
        request = PyRequest("https://example.com")
        print(f"Created request: {request}")
    except (ImportError, AttributeError):
        print("Advanced features not available (using Python fallback)")

if __name__ == "__main__":
    main() 