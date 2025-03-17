use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use tokio::runtime::Runtime;

use scrapy_rs_core::error::Result as ScrapyResult;
use scrapy_rs_core::item::DynamicItem;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::spider::{ParseOutput, Spider};
use scrapy_rs_downloader::{Downloader, DownloaderConfig, HttpDownloader};
use scrapy_rs_engine::{Engine, EngineConfig};
use scrapy_rs_middleware::{
    DefaultHeadersMiddleware, RequestMiddleware, ResponseLoggerMiddleware, ResponseMiddleware,
};
use scrapy_rs_pipeline::{JsonFilePipeline, LogPipeline, Pipeline};
use scrapy_rs_scheduler::{MemoryScheduler, Scheduler};

use crate::common::TestScenario;
use crate::BenchmarkResult;
use crate::BenchmarkRunner;
use crate::{get_cpu_usage, get_memory_usage};

/// Benchmark runner for Scrapy-RS
pub struct ScrapyRsBenchmarkRunner {
    /// Test scenario to run
    scenario: TestScenario,
    /// Name of the benchmark
    name: String,
    /// Output directory for results
    output_dir: Option<String>,
    /// Whether to show progress
    show_progress: bool,
}

impl ScrapyRsBenchmarkRunner {
    /// Create a new Scrapy-RS benchmark runner
    pub fn new(scenario: TestScenario) -> Self {
        Self {
            name: format!("{}_scrapy_rs", scenario.name),
            scenario,
            output_dir: None,
            show_progress: true,
        }
    }

    /// Set the output directory
    pub fn with_output_dir(mut self, dir: &str) -> Self {
        self.output_dir = Some(dir.to_string());
        self
    }

    /// Set whether to show progress
    pub fn with_progress(mut self, show: bool) -> Self {
        self.show_progress = show;
        self
    }

    /// Create a spider for the benchmark
    fn create_spider(&self) -> Arc<dyn Spider> {
        // 创建一个自定义的 Spider 实现
        struct BenchmarkSpider {
            name: String,
            urls: Vec<String>,
            #[allow(dead_code)]
            max_depth: usize,
        }

        impl BenchmarkSpider {
            fn new(name: String, urls: Vec<String>, max_depth: usize) -> Self {
                Self {
                    name,
                    urls,
                    max_depth,
                }
            }
        }

        #[async_trait]
        impl Spider for BenchmarkSpider {
            fn name(&self) -> &str {
                &self.name
            }

            fn start_urls(&self) -> Vec<String> {
                self.urls.clone()
            }

            async fn parse(&self, response: Response) -> ScrapyResult<ParseOutput> {
                let mut output = ParseOutput::new();

                // Create an item for this page
                let mut item = DynamicItem::new("page");
                item.set("url", serde_json::Value::String(response.url.to_string()));

                // Extract title if available
                if let Ok(text) = response.text() {
                    if let Some(title_start) = text.find("<title>") {
                        if let Some(title_end) = text.find("</title>") {
                            let title = &text[title_start + 7..title_end];
                            item.set("title", serde_json::Value::String(title.to_string()));
                        }
                    }

                    // Extract links
                    let mut links = Vec::new();
                    let mut pos = 0;
                    while let Some(href_start) = text[pos..].find("href=\"") {
                        pos += href_start + 6;
                        if let Some(href_end) = text[pos..].find("\"") {
                            let href = &text[pos..pos + href_end];
                            if !href.starts_with("#") && !href.starts_with("javascript:") {
                                if let Ok(url) = response.urljoin(href) {
                                    links.push(url.to_string());

                                    // Create a new request for this URL
                                    if let Ok(request) = Request::get(url.as_str()) {
                                        output.requests.push(request);
                                    }
                                }
                            }
                            pos += href_end + 1;
                        } else {
                            break;
                        }
                    }

                    // Add links to the item
                    let links_json: Vec<serde_json::Value> = links
                        .iter()
                        .map(|l| serde_json::Value::String(l.clone()))
                        .collect();
                    item.set("links", serde_json::Value::Array(links_json));
                }

                output.items.push(item);
                Ok(output)
            }
        }

        Arc::new(BenchmarkSpider::new(
            self.name.clone(),
            self.scenario.urls.clone(),
            self.scenario.max_depth,
        ))
    }

    /// Create a downloader for the benchmark
    fn create_downloader(&self) -> Arc<dyn Downloader> {
        let config = DownloaderConfig {
            concurrent_requests: self.scenario.concurrent_requests,
            user_agent: self.scenario.user_agent.clone(),
            timeout: 30, // 30 seconds timeout
            ..Default::default()
        };

        let downloader = HttpDownloader::new(config).expect("Failed to create downloader");
        Arc::new(downloader)
    }

    /// Create a scheduler for the benchmark
    fn create_scheduler(&self) -> Arc<dyn Scheduler> {
        Arc::new(MemoryScheduler::new())
    }

    /// Create an engine for the benchmark
    fn create_engine(
        &self,
        spider: Arc<dyn Spider>,
        downloader: Arc<dyn Downloader>,
        scheduler: Arc<dyn Scheduler>,
    ) -> Engine {
        // Create engine configuration
        let config = EngineConfig {
            concurrent_requests: self.scenario.concurrent_requests,
            concurrent_items: self.scenario.concurrent_requests,
            download_delay_ms: self.scenario.download_delay_ms,
            user_agent: self.scenario.user_agent.clone(),
            respect_robots_txt: self.scenario.respect_robots_txt,
            follow_redirects: self.scenario.follow_redirects,
            // 注意：如果 EngineConfig 没有 max_requests 字段，可以使用其他字段或跳过
            // max_requests: Some(self.scenario.page_limit),
            ..Default::default()
        };

        // Create pipeline
        let pipeline: Arc<dyn Pipeline> = if let Some(dir) = &self.output_dir {
            let json_path = format!("{}/items_{}.json", dir, self.name);
            // 创建 JSON 文件管道
            let file_pipeline = JsonFilePipeline::new(&json_path, false);
            Arc::new(file_pipeline)
        } else {
            Arc::new(LogPipeline::info())
        };

        // Create request middleware
        let request_middleware: Arc<dyn RequestMiddleware> =
            Arc::new(DefaultHeadersMiddleware::common());

        // Create response middleware
        let response_middleware: Arc<dyn ResponseMiddleware> =
            Arc::new(ResponseLoggerMiddleware::info());

        // Create engine
        Engine::with_components(
            spider,
            scheduler,
            downloader,
            pipeline,
            request_middleware,
            response_middleware,
            config,
        )
    }
}

impl BenchmarkRunner for ScrapyRsBenchmarkRunner {
    fn name(&self) -> &str {
        &self.name
    }

    fn framework(&self) -> &str {
        "scrapy-rs"
    }

    fn run(&mut self) -> BenchmarkResult {
        // Create a new benchmark result
        let mut result = BenchmarkResult::new(&self.name, "scrapy-rs");
        result.start_time = chrono::Utc::now();

        // Create a progress bar if requested
        let progress_bar = if self.show_progress {
            let pb = ProgressBar::new(self.scenario.page_limit as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} pages ({eta})")
                .unwrap()
                .progress_chars("#>-"));
            Some(pb)
        } else {
            None
        };

        // Create a tokio runtime
        let runtime = Runtime::new().expect("Failed to create tokio runtime");

        // Create the spider, downloader, and scheduler
        let spider = self.create_spider();
        let downloader = self.create_downloader();
        let scheduler = self.create_scheduler();

        // Create the engine
        let mut engine = self.create_engine(spider, downloader, scheduler);

        // Record memory and CPU usage before the benchmark
        let memory_before = get_memory_usage();
        let cpu_before = get_cpu_usage();

        // Run the engine and measure the time
        let start_time = Instant::now();

        // If we have a progress bar, update it as the engine runs
        let stats = if let Some(pb) = &progress_bar {
            // 使用 Arc 和 Mutex 来共享 engine 和 runtime
            let engine_arc = Arc::new(tokio::sync::Mutex::new(engine));
            let runtime_arc = Arc::new(runtime);

            // Create a thread to update the progress bar
            let pb_clone = pb.clone();
            let engine_clone = engine_arc.clone();
            let runtime_clone = runtime_arc.clone();

            let handle = std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(100));

                let rt = &runtime_clone;
                let engine_mutex = &engine_clone;

                let is_running = rt.block_on(async {
                    let engine = engine_mutex.lock().await;
                    let stats = engine.stats().await;
                    pb_clone.set_position(stats.request_count as u64);
                    engine.is_running().await
                });

                if !is_running {
                    break;
                }
            });

            // Run the engine
            let stats = runtime_arc
                .block_on(async {
                    let mut engine = engine_arc.lock().await;
                    engine.run().await
                })
                .expect("Failed to run engine");

            // Wait for the progress thread to finish
            handle.join().unwrap();
            pb.finish_with_message("Benchmark completed");

            stats
        } else {
            // Run the engine without progress updates
            runtime
                .block_on(engine.run())
                .expect("Failed to run engine")
        };

        // Update the result with the stats
        result.request_count = stats.request_count;
        result.response_count = stats.response_count;
        result.item_count = stats.item_count;
        result.error_count = stats.error_count;

        // Record the end time and duration
        let duration = start_time.elapsed();
        result.end_time = chrono::Utc::now();
        result.duration_ms = duration.as_millis() as u64;

        // Record memory and CPU usage after the benchmark
        let memory_after = get_memory_usage();
        let cpu_after = get_cpu_usage();

        // Calculate memory and CPU usage
        result.memory_usage_mb = memory_after - memory_before;
        result.cpu_usage_percent = cpu_after - cpu_before;

        // Calculate derived metrics
        result.calculate_metrics();

        // Save the result to a CSV file if an output directory is specified
        if let Some(dir) = &self.output_dir {
            let csv_path = format!("{}/{}_result.csv", dir, self.name);
            if let Err(e) = result.save_to_csv(&csv_path) {
                warn!("Failed to save benchmark result to CSV: {}", e);
            }
        }

        result
    }
}
