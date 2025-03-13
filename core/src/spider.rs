use std::collections::HashMap;
use futures::stream::Stream;
use futures::StreamExt;

use crate::async_trait;
use crate::error::Result;
use crate::item::{Item, DynamicItem};
use crate::request::Request;
use crate::response::Response;

/// Trait for spiders that crawl websites
#[async_trait]
pub trait Spider: Send + Sync + 'static {
    /// Get the name of the spider
    fn name(&self) -> &str;
    
    /// Get the allowed domains for this spider
    fn allowed_domains(&self) -> Vec<String> {
        Vec::new()
    }
    
    /// Get the start URLs for this spider
    fn start_urls(&self) -> Vec<String>;
    
    /// Convert start URLs to requests
    fn start_requests(&self) -> Vec<Result<Request>> {
        self.start_urls()
            .into_iter()
            .map(|url| Request::get(url))
            .collect()
    }
    
    /// Process a response and return items and/or requests
    async fn parse(&self, response: Response) -> Result<ParseOutput>;
    
    /// Called when the spider is closed
    async fn closed(&self) -> Result<()> {
        Ok(())
    }
    
    /// Get custom settings for this spider
    fn settings(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }
}

/// Output from parsing a response
pub struct ParseOutput {
    /// Items extracted from the response
    pub items: Vec<DynamicItem>,
    
    /// Requests to follow
    pub requests: Vec<Request>,
}

impl ParseOutput {
    /// Create a new empty parse output
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            requests: Vec::new(),
        }
    }
    
    /// Add an item to the output
    pub fn add_item<I: Item>(&mut self, item: I) -> &mut Self {
        // Convert any Item to DynamicItem
        let json_value = serde_json::to_value(&item).unwrap_or_default();
        if let serde_json::Value::Object(map) = json_value {
            let mut dynamic_item = DynamicItem::new(item.item_type());
            for (k, v) in map {
                dynamic_item.set(k, v);
            }
            self.items.push(dynamic_item);
        }
        self
    }
    
    /// Add a request to the output
    pub fn add_request(&mut self, request: Request) -> &mut Self {
        self.requests.push(request);
        self
    }
    
    /// Create a parse output with a single item
    pub fn item<I: Item>(item: I) -> Self {
        let mut output = Self::new();
        output.add_item(item);
        output
    }
    
    /// Create a parse output with a single request
    pub fn request(request: Request) -> Self {
        let mut output = Self::new();
        output.add_request(request);
        output
    }
}

impl Default for ParseOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// A basic spider implementation
pub struct BasicSpider {
    /// The name of the spider
    name: String,
    
    /// The allowed domains for this spider
    allowed_domains: Vec<String>,
    
    /// The start URLs for this spider
    start_urls: Vec<String>,
    
    /// Custom settings for this spider
    settings: HashMap<String, serde_json::Value>,
}

impl BasicSpider {
    /// Create a new basic spider
    pub fn new<S: Into<String>>(name: S, start_urls: Vec<String>) -> Self {
        Self {
            name: name.into(),
            allowed_domains: Vec::new(),
            start_urls,
            settings: HashMap::new(),
        }
    }
    
    /// Set the allowed domains for this spider
    pub fn with_allowed_domains(mut self, domains: Vec<String>) -> Self {
        self.allowed_domains = domains;
        self
    }
    
    /// Set a custom setting for this spider
    pub fn with_setting<K: Into<String>, V: Into<serde_json::Value>>(mut self, key: K, value: V) -> Self {
        self.settings.insert(key.into(), value.into());
        self
    }
}

#[async_trait]
impl Spider for BasicSpider {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn allowed_domains(&self) -> Vec<String> {
        self.allowed_domains.clone()
    }
    
    fn start_urls(&self) -> Vec<String> {
        self.start_urls.clone()
    }
    
    async fn parse(&self, _response: Response) -> Result<ParseOutput> {
        // Basic implementation just returns an empty output
        // This should be overridden by users
        Ok(ParseOutput::new())
    }
    
    fn settings(&self) -> HashMap<String, serde_json::Value> {
        self.settings.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::DynamicItem;
    use std::collections::HashMap;

    struct TestSpider {
        name: String,
        start_urls: Vec<String>,
    }

    #[async_trait]
    impl Spider for TestSpider {
        fn name(&self) -> &str {
            &self.name
        }

        fn start_urls(&self) -> Vec<String> {
            self.start_urls.clone()
        }

        async fn parse(&self, response: Response) -> Result<ParseOutput> {
            let mut output = ParseOutput::new();
            
            // Extract a simple item
            let mut item = DynamicItem::new("test");
            item.set("url", response.url.to_string());
            item.set("title", "Test Page");
            
            output.add_item(item);
            
            // Follow a link
            if let Ok(request) = Request::get("https://example.com/next") {
                output.add_request(request);
            }
            
            Ok(output)
        }
    }

    #[tokio::test]
    async fn test_spider_parse() {
        let spider = TestSpider {
            name: "test_spider".to_string(),
            start_urls: vec!["https://example.com".to_string()],
        };

        let request = Request::get("https://example.com").unwrap();
        let response = Response::new(
            request,
            200,
            HashMap::new(),
            Vec::new(),
        );

        let output = spider.parse(response).await.unwrap();
        
        assert_eq!(output.items.len(), 1);
        assert_eq!(output.requests.len(), 1);
        
        let request = &output.requests[0];
        assert_eq!(request.url.as_str(), "https://example.com/next");
    }

    #[test]
    fn test_basic_spider() {
        let spider = BasicSpider::new(
            "basic_spider",
            vec!["https://example.com".to_string()],
        ).with_allowed_domains(vec!["example.com".to_string()])
         .with_setting("download_delay", 2);

        assert_eq!(spider.name(), "basic_spider");
        assert_eq!(spider.allowed_domains(), vec!["example.com"]);
        assert_eq!(spider.start_urls(), vec!["https://example.com"]);
        assert_eq!(spider.settings().get("download_delay").unwrap(), &serde_json::json!(2));
    }
} 