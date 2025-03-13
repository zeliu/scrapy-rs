use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{Error, Result};

/// Trait for items scraped by spiders
pub trait Item: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static {
    /// Get the item type name
    fn item_type(&self) -> &'static str;
    
    /// Convert the item to a HashMap
    fn to_map(&self) -> Result<HashMap<String, serde_json::Value>> {
        let value = serde_json::to_value(self)?;
        match value {
            serde_json::Value::Object(map) => Ok(map.into_iter().collect()),
            _ => Err(Error::item("Item is not an object")),
        }
    }
    
    /// Convert the item to JSON
    fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| Error::SerdeError(e.to_string()))
    }
}

/// A dynamic item that can hold any key-value pairs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicItem {
    /// The type of the item
    #[serde(rename = "_type")]
    pub item_type_name: String,
    
    /// The fields of the item
    #[serde(flatten)]
    pub fields: HashMap<String, serde_json::Value>,
}

impl DynamicItem {
    /// Create a new dynamic item
    pub fn new<S: Into<String>>(item_type_name: S) -> Self {
        Self {
            item_type_name: item_type_name.into(),
            fields: HashMap::new(),
        }
    }

    /// Set a field value
    pub fn set<K: Into<String>, V: Into<serde_json::Value>>(&mut self, key: K, value: V) -> &mut Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    /// Get a field value
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.fields.get(key)
    }

    /// Check if a field exists
    pub fn has_field(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }

    /// Get a field value as a JSON object
    pub fn as_object(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        let value = serde_json::to_value(self)?;
        if let Some(obj) = value.as_object() {
            Ok(obj.clone())
        } else {
            Err(Error::item("Item is not an object"))
        }
    }
}

impl Item for DynamicItem {
    fn item_type(&self) -> &'static str {
        "dynamic_item"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestItem {
        title: String,
        price: f64,
        tags: Vec<String>,
    }

    impl Item for TestItem {
        fn item_type(&self) -> &'static str {
            "test_item"
        }
    }

    #[test]
    fn test_item_to_map() {
        let item = TestItem {
            title: "Test Product".to_string(),
            price: 19.99,
            tags: vec!["test".to_string(), "product".to_string()],
        };

        let map = item.to_map().unwrap();
        assert_eq!(map.get("title").unwrap(), &json!("Test Product"));
        assert_eq!(map.get("price").unwrap(), &json!(19.99));
        assert_eq!(map.get("tags").unwrap(), &json!(["test", "product"]));
    }

    #[test]
    fn test_item_to_json() {
        let item = TestItem {
            title: "Test Product".to_string(),
            price: 19.99,
            tags: vec!["test".to_string(), "product".to_string()],
        };

        let json_str = item.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        
        assert_eq!(parsed["title"], "Test Product");
        assert_eq!(parsed["price"], 19.99);
        assert_eq!(parsed["tags"], json!(["test", "product"]));
    }

    #[test]
    fn test_dynamic_item() {
        let mut item = DynamicItem::new("product");
        item.set("title", "Test Product")
            .set("price", 19.99)
            .set("tags", json!(["test", "product"]));

        assert_eq!(item.get("title").unwrap(), &json!("Test Product"));
        assert_eq!(item.get("price").unwrap(), &json!(19.99));
        assert!(item.has_field("tags"));
        assert!(!item.has_field("description"));
    }
} 