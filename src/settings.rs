use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for settings operations
#[derive(Error, Debug)]
pub enum SettingsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(String),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    YamlParse(String),

    #[error("Python parse error: {0}")]
    PythonParse(String),

    #[error("Unknown file format: {0}")]
    UnknownFormat(String),

    #[error("Setting not found: {0}")]
    SettingNotFound(String),
}

/// Result type for settings operations
pub type Result<T> = std::result::Result<T, SettingsError>;

/// Settings format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsFormat {
    /// TOML format
    Toml,
    /// JSON format
    Json,
    /// YAML format
    Yaml,
    /// Python format
    Python,
}

impl SettingsFormat {
    /// Detect the format from a file path
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension().and_then(|ext| {
            let ext = ext.to_string_lossy().to_lowercase();
            match ext.as_str() {
                "toml" => Some(Self::Toml),
                "json" => Some(Self::Json),
                "yaml" | "yml" => Some(Self::Yaml),
                "py" => Some(Self::Python),
                _ => None,
            }
        })
    }
}

/// Settings for the crawler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Raw settings as key-value pairs
    #[serde(flatten)]
    pub raw: HashMap<String, serde_json::Value>,

    /// Path to the settings file, if loaded from a file
    #[serde(skip)]
    pub file_path: Option<PathBuf>,
}
#[allow(clippy::derivable_impls)]
impl Default for Settings {
    fn default() -> Self {
        Self {
            raw: HashMap::new(),
            file_path: None,
        }
    }
}

impl Settings {
    /// Create a new empty settings object
    pub fn new() -> Self {
        Self::default()
    }

    /// Load settings from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let format = SettingsFormat::from_path(path)
            .ok_or_else(|| SettingsError::UnknownFormat(path.to_string_lossy().to_string()))?;

        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let mut settings = match format {
            SettingsFormat::Toml => Self::from_toml(&contents)?,
            SettingsFormat::Json => Self::from_json(&contents)?,
            SettingsFormat::Yaml => Self::from_yaml(&contents)?,
            SettingsFormat::Python => Self::from_python(&contents)?,
        };

        settings.file_path = Some(path.to_path_buf());
        Ok(settings)
    }

    /// Load settings from TOML
    pub fn from_toml(contents: &str) -> Result<Self> {
        let raw: HashMap<String, serde_json::Value> =
            toml::from_str(contents).map_err(|e| SettingsError::TomlParse(e.to_string()))?;
        Ok(Self {
            raw,
            file_path: None,
        })
    }

    /// Load settings from JSON
    pub fn from_json(contents: &str) -> Result<Self> {
        let raw: HashMap<String, serde_json::Value> = serde_json::from_str(contents)?;
        Ok(Self {
            raw,
            file_path: None,
        })
    }

    /// Load settings from YAML
    pub fn from_yaml(_contents: &str) -> Result<Self> {
        #[cfg(feature = "yaml")]
        {
            let raw: HashMap<String, serde_json::Value> = serde_yaml::from_str(_contents)
                .map_err(|e| SettingsError::YamlParse(e.to_string()))?;
            Ok(Self {
                raw,
                file_path: None,
            })
        }

        #[cfg(not(feature = "yaml"))]
        {
            Err(SettingsError::YamlParse(
                "YAML support not enabled".to_string(),
            ))
        }
    }

    /// Load settings from Python
    pub fn from_python(contents: &str) -> Result<Self> {
        // This is a very simple parser for Python settings files
        // It only supports simple assignments like KEY = VALUE
        let mut raw = HashMap::new();

        for line in contents.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse assignments
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim();

                // Parse the value
                let value = if value == "True" || value == "true" {
                    serde_json::Value::Bool(true)
                } else if value == "False" || value == "false" {
                    serde_json::Value::Bool(false)
                } else if value == "None" || value == "null" {
                    serde_json::Value::Null
                } else if let Ok(num) = value.parse::<i64>() {
                    serde_json::Value::Number(num.into())
                } else if let Ok(num) = value.parse::<f64>() {
                    // This is a bit tricky because serde_json::Number doesn't have a from_f64 method
                    if let Some(num) = serde_json::Number::from_f64(num) {
                        serde_json::Value::Number(num)
                    } else {
                        serde_json::Value::String(value.to_string())
                    }
                } else if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    // String with quotes
                    let value = &value[1..value.len() - 1];
                    serde_json::Value::String(value.to_string())
                } else if value.starts_with('[') && value.ends_with(']') {
                    // List
                    let value = &value[1..value.len() - 1];
                    let items: Vec<&str> = value
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();

                    let items: Vec<serde_json::Value> = items
                        .iter()
                        .map(|s| {
                            if *s == "True" || *s == "true" {
                                serde_json::Value::Bool(true)
                            } else if *s == "False" || *s == "false" {
                                serde_json::Value::Bool(false)
                            } else if *s == "None" || *s == "null" {
                                serde_json::Value::Null
                            } else if let Ok(num) = s.parse::<i64>() {
                                serde_json::Value::Number(num.into())
                            } else if let Ok(num) = s.parse::<f64>() {
                                if let Some(num) = serde_json::Number::from_f64(num) {
                                    serde_json::Value::Number(num)
                                } else {
                                    serde_json::Value::String(s.to_string())
                                }
                            } else if (s.starts_with('"') && s.ends_with('"'))
                                || (s.starts_with('\'') && s.ends_with('\''))
                            {
                                let s = &s[1..s.len() - 1];
                                serde_json::Value::String(s.to_string())
                            } else {
                                serde_json::Value::String(s.to_string())
                            }
                        })
                        .collect();

                    serde_json::Value::Array(items)
                } else {
                    // Assume it's a string
                    serde_json::Value::String(value.to_string())
                };

                raw.insert(key, value);
            }
        }

        Ok(Self {
            raw,
            file_path: None,
        })
    }

    /// Get a setting as a specific type
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<T> {
        self.raw
            .get(key)
            .ok_or_else(|| SettingsError::SettingNotFound(key.to_string()))
            .and_then(|value| {
                serde_json::from_value(value.clone()).map_err(SettingsError::JsonParse)
            })
    }

    /// Get a setting with a default value
    pub fn get_or<T: for<'de> Deserialize<'de>>(&self, key: &str, default: T) -> T {
        self.get(key).unwrap_or(default)
    }

    /// Set a setting
    pub fn set<T: Serialize>(&mut self, key: &str, value: T) -> Result<()> {
        let value = serde_json::to_value(value).map_err(SettingsError::JsonParse)?;
        self.raw.insert(key.to_string(), value);
        Ok(())
    }

    /// Check if a setting exists
    pub fn contains(&self, key: &str) -> bool {
        self.raw.contains_key(key)
    }

    /// Remove a setting
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.raw.remove(key)
    }

    /// Get all settings
    pub fn all(&self) -> &HashMap<String, serde_json::Value> {
        &self.raw
    }

    /// Save settings to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let format = SettingsFormat::from_path(path)
            .ok_or_else(|| SettingsError::UnknownFormat(path.to_string_lossy().to_string()))?;

        let contents = match format {
            SettingsFormat::Toml => {
                toml::to_string(&self.raw).map_err(|e| SettingsError::TomlParse(e.to_string()))?
            }
            SettingsFormat::Json => {
                serde_json::to_string_pretty(&self.raw).map_err(SettingsError::JsonParse)?
            }
            SettingsFormat::Yaml => {
                #[cfg(feature = "yaml")]
                {
                    serde_yaml::to_string(&self.raw)
                        .map_err(|e| SettingsError::YamlParse(e.to_string()))?
                }

                #[cfg(not(feature = "yaml"))]
                {
                    return Err(SettingsError::YamlParse(
                        "YAML support not enabled".to_string(),
                    ));
                }
            }
            SettingsFormat::Python => {
                let mut result = String::new();
                result.push_str("# Scrapy-RS settings\n\n");

                for (key, value) in &self.raw {
                    let value_str = match value {
                        serde_json::Value::Null => "None".to_string(),
                        serde_json::Value::Bool(b) => {
                            if *b {
                                "True".to_string()
                            } else {
                                "False".to_string()
                            }
                        }
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::String(s) => format!("'{}'", s),
                        serde_json::Value::Array(a) => {
                            let items: Vec<String> = a
                                .iter()
                                .map(|v| match v {
                                    serde_json::Value::Null => "None".to_string(),
                                    serde_json::Value::Bool(b) => {
                                        if *b {
                                            "True".to_string()
                                        } else {
                                            "False".to_string()
                                        }
                                    }
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::String(s) => format!("'{}'", s),
                                    _ => format!("'{}'", v),
                                })
                                .collect();
                            format!("[{}]", items.join(", "))
                        }
                        serde_json::Value::Object(_) => format!("'{}'", value),
                    };

                    result.push_str(&format!("{} = {}\n", key, value_str));
                }

                result
            }
        };

        std::fs::write(path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_toml() {
        let toml = r#"
            concurrent_requests = 16
            download_delay_ms = 100
            user_agent = "scrapy_rs/0.1.0"
            follow_redirects = true
        "#;

        let settings = Settings::from_toml(toml).unwrap();

        assert_eq!(settings.get::<usize>("concurrent_requests").unwrap(), 16);
        assert_eq!(settings.get::<u64>("download_delay_ms").unwrap(), 100);
        assert_eq!(
            settings.get::<String>("user_agent").unwrap(),
            "scrapy_rs/0.1.0"
        );
        assert!(settings.get::<bool>("follow_redirects").unwrap());
    }

    #[test]
    fn test_from_json() {
        let json = r#"
        {
            "concurrent_requests": 16,
            "download_delay_ms": 100,
            "user_agent": "scrapy_rs/0.1.0",
            "follow_redirects": true
        }
        "#;

        let settings = Settings::from_json(json).unwrap();

        assert_eq!(settings.get::<usize>("concurrent_requests").unwrap(), 16);
        assert_eq!(settings.get::<u64>("download_delay_ms").unwrap(), 100);
        assert_eq!(
            settings.get::<String>("user_agent").unwrap(),
            "scrapy_rs/0.1.0"
        );
        assert!(settings.get::<bool>("follow_redirects").unwrap());
    }

    #[test]
    fn test_from_python() {
        let python = r#"
            # Scrapy-RS settings
            
            CONCURRENT_REQUESTS = 16
            DOWNLOAD_DELAY_MS = 100
            USER_AGENT = 'scrapy_rs/0.1.0'
            FOLLOW_REDIRECTS = True
            ALLOWED_DOMAINS = ['example.com', 'example.org']
        "#;

        let settings = Settings::from_python(python).unwrap();

        assert_eq!(settings.get::<usize>("CONCURRENT_REQUESTS").unwrap(), 16);
        assert_eq!(settings.get::<u64>("DOWNLOAD_DELAY_MS").unwrap(), 100);
        assert_eq!(
            settings.get::<String>("USER_AGENT").unwrap(),
            "scrapy_rs/0.1.0"
        );
        assert!(settings.get::<bool>("FOLLOW_REDIRECTS").unwrap());

        let allowed_domains: Vec<String> = settings.get("ALLOWED_DOMAINS").unwrap();
        assert_eq!(allowed_domains, vec!["example.com", "example.org"]);
    }
}
