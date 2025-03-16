#!/usr/bin/env python3
"""
Example script to demonstrate the enhanced Response features in Scrapy-RS
"""

import scrapy_rs

def main():
    try:
        from scrapy_rs import PyRequest, PyResponse
        
        # Create a request
        request = PyRequest("https://example.com")
        
        # Create a simple response
        response = PyResponse(
            url="https://example.com",
            status=200,
            headers={"Content-Type": "text/html", "Server": "nginx"},
            body=b"<html><body><h1>Example Page</h1><a href='/page1'>Link 1</a><a href='/page2'>Link 2</a></body></html>",
            request=request
        )
        
        print(f"Created response: {response}")
        print(f"URL: {response.url}")
        print(f"Status: {response.status}")
        print(f"Headers: {response.headers}")
        print(f"Body length: {len(response.body)} bytes")
        print(f"Text content: {response.text()}")
        
        # Demonstrate flags
        response.add_flag("cached")
        response.add_flag("from-middleware")
        print(f"\nFlags: {response.flags}")
        print(f"Has 'cached' flag: {response.has_flag('cached')}")
        print(f"Has 'non-existent' flag: {response.has_flag('non-existent')}")
        
        # Demonstrate URL joining
        joined_url = response.urljoin("/relative/path")
        print(f"\nJoined URL: {joined_url}")
        
        # Demonstrate follow
        followed_request = response.follow("/page1")
        print(f"\nFollowed request: {followed_request}")
        print(f"Followed URL: {followed_request.url}")
        
        # Demonstrate follow_all
        followed_requests = response.follow_all(["/page1", "/page2", "/page3"])
        print(f"\nFollowed {len(followed_requests)} requests:")
        for i, req in enumerate(followed_requests):
            print(f"  {i+1}. {req.url}")
        
        # Demonstrate copy
        copied_response = response.copy()
        print(f"\nCopied response: {copied_response}")
        print(f"Same as original: {copied_response.url == response.url and copied_response.status == response.status}")
        
        # Demonstrate replace
        replaced_response = response.replace(
            status=404,
            body=b"<html><body><h1>Not Found</h1></body></html>",
            flags=["error", "not-found"]
        )
        print(f"\nReplaced response: {replaced_response}")
        print(f"New status: {replaced_response.status}")
        print(f"New flags: {replaced_response.flags}")
        print(f"New body: {replaced_response.text()}")
        
        # Demonstrate additional properties
        response_with_props = response.replace(
            ip_address="192.168.1.1",
            protocol="https",
            certificate=b"dummy-certificate-data"
        )
        print(f"\nResponse with properties:")
        print(f"IP Address: {response_with_props.ip_address}")
        print(f"Protocol: {response_with_props.protocol}")
        print(f"Has certificate: {response_with_props.certificate is not None}")
        
    except (ImportError, AttributeError) as e:
        print(f"Advanced features not available (using Python fallback): {e}")

if __name__ == "__main__":
    main() 