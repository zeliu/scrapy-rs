use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use log::{debug, error, info};
use scrapy_rs_core::async_trait;
use scrapy_rs_core::error::{Error, Result};
use scrapy_rs_core::item::{DynamicItem, Item};
use scrapy_rs_core::spider::Spider;
use tokio::sync::Mutex;

/// Trait for item pipelines
#[async_trait]
pub trait Pipeline: Send + Sync + 'static {
    /// Process a dynamic item
    async fn process_dynamic_item(
        &self,
        item: DynamicItem,
        spider: &dyn Spider,
    ) -> Result<DynamicItem>;

    /// Called when the spider is opened
    async fn open_spider(&self, _spider: &dyn Spider) -> Result<()> {
        Ok(())
    }

    /// Called when the spider is closed
    async fn close_spider(&self, _spider: &dyn Spider) -> Result<()> {
        Ok(())
    }
}

/// A no-op pipeline that passes items through unchanged
pub struct DummyPipeline;

impl DummyPipeline {
    /// Create a new dummy pipeline
    pub fn new() -> Self {
        Self
    }
}

impl Default for DummyPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Pipeline for DummyPipeline {
    async fn process_dynamic_item(
        &self,
        item: DynamicItem,
        _spider: &dyn Spider,
    ) -> Result<DynamicItem> {
        Ok(item)
    }
}

/// Pipeline that logs items
pub struct LogPipeline {
    /// Log level to use
    level: log::Level,
}

impl LogPipeline {
    /// Create a new log pipeline
    pub fn new(level: log::Level) -> Self {
        Self { level }
    }

    /// Create a new log pipeline with INFO level
    pub fn info() -> Self {
        Self::new(log::Level::Info)
    }

    /// Create a new log pipeline with DEBUG level
    pub fn debug() -> Self {
        Self::new(log::Level::Debug)
    }
}

#[async_trait]
impl Pipeline for LogPipeline {
    async fn process_dynamic_item(
        &self,
        item: DynamicItem,
        spider: &dyn Spider,
    ) -> Result<DynamicItem> {
        match self.level {
            log::Level::Error => error!("Spider '{}' scraped item: {:?}", spider.name(), item),
            log::Level::Warn => log::warn!("Spider '{}' scraped item: {:?}", spider.name(), item),
            log::Level::Info => info!("Spider '{}' scraped item: {:?}", spider.name(), item),
            log::Level::Debug => debug!("Spider '{}' scraped item: {:?}", spider.name(), item),
            log::Level::Trace => log::trace!("Spider '{}' scraped item: {:?}", spider.name(), item),
        }

        Ok(item)
    }
}

/// Pipeline that writes items to a JSON file
pub struct JsonFilePipeline {
    /// Path to the output file
    file_path: String,

    /// File handle
    file: Arc<Mutex<Option<File>>>,

    /// Whether to append to the file
    append: bool,
}

impl JsonFilePipeline {
    /// Create a new JSON file pipeline
    pub fn new<P: AsRef<Path>>(file_path: P, append: bool) -> Self {
        Self {
            file_path: file_path.as_ref().to_string_lossy().to_string(),
            file: Arc::new(Mutex::new(None)),
            append,
        }
    }
}

#[async_trait]
impl Pipeline for JsonFilePipeline {
    async fn open_spider(&self, _spider: &dyn Spider) -> Result<()> {
        let mut file_guard = self.file.lock().await;

        let file = if self.append {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)
                .map_err(|e| Error::IoError(e.to_string()))?
        } else {
            File::create(&self.file_path).map_err(|e| Error::IoError(e.to_string()))?
        };

        *file_guard = Some(file);

        // If we're not appending, write the opening bracket for the JSON array
        if !self.append {
            if let Some(file) = file_guard.as_mut() {
                file.write_all(b"[\n")
                    .map_err(|e| Error::IoError(e.to_string()))?;
            }
        }

        Ok(())
    }

    async fn process_dynamic_item(
        &self,
        item: DynamicItem,
        _spider: &dyn Spider,
    ) -> Result<DynamicItem> {
        let mut file_guard = self.file.lock().await;

        if let Some(file) = file_guard.as_mut() {
            // Convert item to JSON
            let json = serde_json::to_string_pretty(&item)
                .map_err(|e| Error::SerdeError(e.to_string()))?;

            // Write the item to the file
            file.write_all(json.as_bytes())
                .map_err(|e| Error::IoError(e.to_string()))?;
            file.write_all(b",\n")
                .map_err(|e| Error::IoError(e.to_string()))?;
            file.flush().map_err(|e| Error::IoError(e.to_string()))?;
        }

        Ok(item)
    }

    async fn close_spider(&self, _spider: &dyn Spider) -> Result<()> {
        let mut file_guard = self.file.lock().await;

        if let Some(mut file) = file_guard.take() {
            // Seek to the end of the file minus 2 bytes (to overwrite the last comma and newline)
            let pos = file
                .metadata()
                .map_err(|e| Error::IoError(e.to_string()))?
                .len();
            if pos > 2 {
                use std::io::Seek;
                file.seek(std::io::SeekFrom::End(-2))
                    .map_err(|e| Error::IoError(e.to_string()))?;
            }

            // Write the closing bracket for the JSON array
            file.write_all(b"\n]")
                .map_err(|e| Error::IoError(e.to_string()))?;
            file.flush().map_err(|e| Error::IoError(e.to_string()))?;
        }

        Ok(())
    }
}

/// Pipeline that filters items based on a predicate
pub struct FilterPipeline<F>
where
    F: Fn(&DynamicItem) -> bool + Send + Sync + 'static,
{
    /// The filter predicate
    filter: F,
}

impl<F> FilterPipeline<F>
where
    F: Fn(&DynamicItem) -> bool + Send + Sync + 'static,
{
    /// Create a new filter pipeline
    pub fn new(filter: F) -> Self {
        Self { filter }
    }
}

#[async_trait]
impl<F> Pipeline for FilterPipeline<F>
where
    F: Fn(&DynamicItem) -> bool + Send + Sync + 'static,
{
    async fn process_dynamic_item(
        &self,
        item: DynamicItem,
        _spider: &dyn Spider,
    ) -> Result<DynamicItem> {
        if (self.filter)(&item) {
            Ok(item)
        } else {
            Err(Box::new(Error::item("Item filtered out")))
        }
    }
}

/// Enum of all pipeline types
pub enum PipelineType {
    Dummy(DummyPipeline),
    Log(LogPipeline),
    JsonFile(JsonFilePipeline),
    #[allow(clippy::type_complexity)]
    Filter(FilterPipeline<Box<dyn Fn(&DynamicItem) -> bool + Send + Sync + 'static>>),
    Chained(Vec<PipelineType>),
}

#[async_trait]
impl Pipeline for PipelineType {
    async fn process_dynamic_item(
        &self,
        item: DynamicItem,
        spider: &dyn Spider,
    ) -> Result<DynamicItem> {
        match self {
            PipelineType::Dummy(p) => p.process_dynamic_item(item, spider).await,
            PipelineType::Log(p) => p.process_dynamic_item(item, spider).await,
            PipelineType::JsonFile(p) => p.process_dynamic_item(item, spider).await,
            PipelineType::Filter(p) => p.process_dynamic_item(item, spider).await,
            PipelineType::Chained(pipelines) => {
                let mut current_item = item;
                for pipeline in pipelines {
                    current_item = pipeline.process_dynamic_item(current_item, spider).await?;
                }
                Ok(current_item)
            }
        }
    }

    async fn open_spider(&self, spider: &dyn Spider) -> Result<()> {
        match self {
            PipelineType::Dummy(p) => p.open_spider(spider).await,
            PipelineType::Log(p) => p.open_spider(spider).await,
            PipelineType::JsonFile(p) => p.open_spider(spider).await,
            PipelineType::Filter(p) => p.open_spider(spider).await,
            PipelineType::Chained(pipelines) => {
                for pipeline in pipelines {
                    pipeline.open_spider(spider).await?;
                }
                Ok(())
            }
        }
    }

    async fn close_spider(&self, spider: &dyn Spider) -> Result<()> {
        match self {
            PipelineType::Dummy(p) => p.close_spider(spider).await,
            PipelineType::Log(p) => p.close_spider(spider).await,
            PipelineType::JsonFile(p) => p.close_spider(spider).await,
            PipelineType::Filter(p) => p.close_spider(spider).await,
            PipelineType::Chained(pipelines) => {
                for pipeline in pipelines {
                    pipeline.close_spider(spider).await?;
                }
                Ok(())
            }
        }
    }
}

/// Helper function to convert any Item to DynamicItem
pub fn to_dynamic_item<I: Item>(item: &I) -> Result<DynamicItem> {
    let json_value = serde_json::to_value(item)?;
    if let serde_json::Value::Object(map) = json_value {
        let mut dynamic_item = DynamicItem::new(item.item_type());
        for (k, v) in map {
            dynamic_item.set(k, v);
        }
        Ok(dynamic_item)
    } else {
        Err(Box::new(Error::item(
            "Failed to convert item to DynamicItem",
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scrapy_rs_core::item::DynamicItem;

    struct TestSpider {
        name: String,
    }

    #[async_trait]
    impl Spider for TestSpider {
        fn name(&self) -> &str {
            &self.name
        }

        fn start_urls(&self) -> Vec<String> {
            vec![]
        }

        async fn parse(
            &self,
            _response: scrapy_rs_core::response::Response,
        ) -> Result<scrapy_rs_core::spider::ParseOutput> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_dummy_pipeline() {
        let pipeline = DummyPipeline::new();
        let spider = TestSpider {
            name: "test".to_string(),
        };

        let mut item = DynamicItem::new("test");
        item.set("field1", "value1");

        let result = pipeline
            .process_dynamic_item(item.clone(), &spider)
            .await
            .unwrap();
        assert_eq!(result.get("field1").unwrap().as_str().unwrap(), "value1");
    }

    #[tokio::test]
    async fn test_filter_pipeline() {
        let allow_pipeline = FilterPipeline::new(|item: &DynamicItem| item.has_field("keep"));

        let deny_pipeline = FilterPipeline::new(|item: &DynamicItem| !item.has_field("drop"));

        let spider = TestSpider {
            name: "test".to_string(),
        };

        let mut keep_item = DynamicItem::new("test");
        keep_item.set("keep", true);

        let mut drop_item = DynamicItem::new("test");
        drop_item.set("drop", true);

        // Test allow pipeline
        let result = allow_pipeline
            .process_dynamic_item(keep_item.clone(), &spider)
            .await
            .unwrap();
        assert!(result.get("keep").unwrap().as_bool().unwrap());

        let result = allow_pipeline
            .process_dynamic_item(drop_item.clone(), &spider)
            .await;
        assert!(result.is_err());

        // Test deny pipeline
        let result = deny_pipeline
            .process_dynamic_item(keep_item.clone(), &spider)
            .await
            .unwrap();
        assert!(result.get("keep").unwrap().as_bool().unwrap());

        let result = deny_pipeline
            .process_dynamic_item(drop_item.clone(), &spider)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pipeline_enum() {
        let dummy = PipelineType::Dummy(DummyPipeline::new());
        let log = PipelineType::Log(LogPipeline::info());
        let filter = PipelineType::Filter(FilterPipeline {
            filter: Box::new(|item: &DynamicItem| item.has_field("test")),
        });

        let spider = TestSpider {
            name: "test".to_string(),
        };

        let mut item = DynamicItem::new("test");
        item.set("test", "value");

        let result = dummy
            .process_dynamic_item(item.clone(), &spider)
            .await
            .unwrap();
        assert_eq!(result.get("test").unwrap().as_str().unwrap(), "value");

        let result = log
            .process_dynamic_item(item.clone(), &spider)
            .await
            .unwrap();
        assert_eq!(result.get("test").unwrap().as_str().unwrap(), "value");

        let result = filter
            .process_dynamic_item(item.clone(), &spider)
            .await
            .unwrap();
        assert_eq!(result.get("test").unwrap().as_str().unwrap(), "value");

        let chained = PipelineType::Chained(vec![dummy, log, filter]);
        let result = chained
            .process_dynamic_item(item.clone(), &spider)
            .await
            .unwrap();
        assert_eq!(result.get("test").unwrap().as_str().unwrap(), "value");
    }
}
