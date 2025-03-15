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
        
        # Create a basic request
        request = PyRequest("https://example.com")
        print(f"Created request: {request}")
        
        # Demonstrate new features
        
        # Add cookies
        request.set_cookie("session", "abc123")
        request.set_cookie("user", "john_doe")
        print(f"Cookies: {request.cookies}")
        
        # Set dont_filter flag
        request.set_dont_filter(True)
        print(f"Dont filter: {request.dont_filter}")
        
        # Set timeout
        request.set_timeout(30.5)  # 30.5 seconds
        print(f"Timeout: {request.timeout} seconds")
        
        # Set encoding
        request.set_encoding("utf-8")
        print(f"Encoding: {request.encoding}")
        
        # Add flags
        request.add_flag("high-priority")
        request.add_flag("no-cache")
        print(f"Flags: {request.flags}")
        print(f"Has 'high-priority' flag: {request.has_flag('high-priority')}")
        print(f"Has 'non-existent' flag: {request.has_flag('non-existent')}")
        
        # Set proxy
        request.set_proxy("http://proxy.example.com:8080")
        print(f"Proxy: {request.proxy}")
        
        # Create a more complex request with all features at once
        complex_request = PyRequest(
            url="https://api.example.com/data",
            method="POST",
            headers={"Content-Type": "application/json", "User-Agent": "scrapy-rs/0.1.0"},
            body=b'{"query": "example"}',
            cookies={"session": "xyz789"},
            dont_filter=True,
            timeout=10.0,
            encoding="utf-8",
            flags=["api-request", "retry-3"],
            proxy="http://proxy.example.com:8080"
        )
        print(f"\nComplex request: {complex_request}")
        print(f"Method: {complex_request.method}")
        print(f"Headers: {complex_request.headers}")
        print(f"Cookies: {complex_request.cookies}")
        print(f"Flags: {complex_request.flags}")
        
    except (ImportError, AttributeError) as e:
        print(f"Advanced features not available (using Python fallback): {e}")

if __name__ == "__main__":
    main() 