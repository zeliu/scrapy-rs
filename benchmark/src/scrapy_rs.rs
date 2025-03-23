use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, warn};
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
use crate::mock_server::{MockServer, MockServerConfig};
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
    /// Whether to use a mock server
    use_mock_server: bool,
    /// Mock server configuration
    mock_server_config: Option<MockServerConfig>,
    /// Maximum run time in seconds (0 means no limit)
    max_run_time_seconds: u64,
}

impl ScrapyRsBenchmarkRunner {
    /// Create a new Scrapy-RS benchmark runner
    pub fn new(scenario: TestScenario) -> Self {
        Self {
            name: format!("{}_scrapy_rs", scenario.name),
            scenario,
            output_dir: None,
            show_progress: true,
            use_mock_server: false,
            mock_server_config: None,
            max_run_time_seconds: 0, // No time limit by default
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

    /// Configure the benchmark to use a mock server
    pub fn with_mock_server(mut self, config: MockServerConfig) -> Self {
        self.use_mock_server = true;
        self.mock_server_config = Some(config);
        self
    }

    /// Set the maximum run time in seconds
    pub fn with_max_run_time(mut self, seconds: u64) -> Self {
        self.max_run_time_seconds = seconds;
        self
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
            // Note: If EngineConfig doesn't have a max_requests field, use other fields or skip
            // max_requests: Some(self.scenario.page_limit),
            ..Default::default()
        };

        // Create pipeline
        let pipeline: Arc<dyn Pipeline> = if let Some(dir) = &self.output_dir {
            let json_path = format!("{}/items_{}.json", dir, self.name);
            // Create JSON file pipeline
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

    // Add new helper method to create a spider with a specific scenario
    fn create_spider_with_scenario(&self, scenario: TestScenario) -> Arc<dyn Spider> {
        // Create a basic spider
        struct BenchmarkSpider {
            name: String,
            start_urls: Vec<String>,
            page_limit: usize,
            max_depth: u32,
            links_per_page: usize,
        }

        #[async_trait]
        impl Spider for BenchmarkSpider {
            fn name(&self) -> &str {
                &self.name
            }

            fn start_urls(&self) -> Vec<String> {
                self.start_urls.clone()
            }

            async fn parse(&self, response: Response) -> ScrapyResult<ParseOutput> {
                let mut output = ParseOutput::new();

                // Extract depth from meta, defaulting to 0
                let depth = match response.meta.get("depth") {
                    Some(value) => {
                        if let Some(d) = value.as_u64() {
                            d as u32
                        } else {
                            0
                        }
                    }
                    None => 0,
                };

                // Create an item from the response
                let mut item = DynamicItem::new("benchmark_item");
                item.set("url", response.url.to_string());
                item.set("title", "Benchmark Page".to_string());
                item.set("depth", depth);
                output.add_item(item);

                // Follow links if we haven't reached the limit
                if depth < self.max_depth {
                    // Extract links and follow them
                    let base_url = response.url.origin().ascii_serialization();

                    // For mock server, links are in the format /<id> (e.g. /0, /1, /2)
                    for i in 1..=self.links_per_page {
                        // Don't exceed page limit
                        if output.requests.len() + 1 >= self.page_limit {
                            break;
                        }

                        // Create a new URL using the correct mock server format
                        let url = format!("{}/{}", base_url, i);
                        let mut request = Request::get(&url).unwrap();
                        // Store depth as a JSON number
                        request
                            .meta
                            .insert("depth".to_string(), serde_json::json!(depth + 1));
                        output.add_request(request);
                    }
                }

                Ok(output)
            }

            // Implement the closed method to ensure proper cleanup
            async fn closed(&self) -> ScrapyResult<()> {
                info!("BenchmarkSpider closed cleanly");
                Ok(())
            }
        }

        // Determine the links per page from mock server config if available
        let links_per_page = if self.use_mock_server {
            self.mock_server_config
                .as_ref()
                .map_or(5, |config| config.links_per_page)
        } else {
            // Default value if not using mock server
            5
        };

        // Create a spider instance
        let spider = BenchmarkSpider {
            name: scenario.name.clone(),
            start_urls: scenario.urls.clone(),
            page_limit: scenario.page_limit,
            max_depth: scenario.max_depth as u32,
            links_per_page,
        };

        Arc::new(spider)
    }
}

impl BenchmarkRunner for ScrapyRsBenchmarkRunner {
    fn name(&self) -> &str {
        &self.name
    }

    fn framework(&self) -> &str {
        "scrapy-rs"
    }

    /// Run the benchmark
    fn run(&mut self) -> BenchmarkResult {
        // Create a new benchmark result
        let mut result = BenchmarkResult::new(&self.name, "scrapy-rs");
        result.start_time = chrono::Utc::now();

        // Create a progress bar if needed
        let progress_bar = if self.show_progress {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
            );
            pb.set_message("Running benchmark...");
            Some(pb)
        } else {
            None
        };

        // Start the mock server if configured
        let mut mock_server: Option<(MockServer, tokio::runtime::Runtime)> = None;
        let _original_urls = self.scenario.urls.clone();
        let mut modified_scenario = self.scenario.clone();

        if self.use_mock_server {
            if let Some(config) = &self.mock_server_config {
                if let Some(pb) = &progress_bar {
                    pb.set_message("Starting mock server...");
                }

                let mut server = MockServer::new(config.clone());
                let runtime = tokio::runtime::Runtime::new().unwrap();
                let base_url =
                    runtime.block_on(async { server.start(config.clone()).await.unwrap() });

                // Replace the URLs with the mock server URL
                modified_scenario.urls = vec![format!("{}/0", base_url)];

                mock_server = Some((server, runtime));

                if let Some(pb) = &progress_bar {
                    pb.set_message("Mock server started");
                }
            }
        }

        // Record memory and CPU usage before the benchmark
        let memory_before = get_memory_usage();
        let cpu_before = get_cpu_usage();

        // Create a runtime
        let rt = Runtime::new().unwrap();

        // Create a spider, downloader, and scheduler
        // Use the modified scenario if we're using a mock server
        let scenario_to_use = if self.use_mock_server && mock_server.is_some() {
            &modified_scenario
        } else {
            &self.scenario
        };

        // Create components with the appropriate scenario
        let spider = self.create_spider_with_scenario(scenario_to_use.clone());
        let downloader = self.create_downloader();
        let scheduler = self.create_scheduler();

        // Create and run the engine
        let mut engine = self.create_engine(spider.clone(), downloader, scheduler);

        // Record the start time
        let start_time = Instant::now();

        // Update progress bar
        if let Some(pb) = &progress_bar {
            pb.set_message("Crawling...");
        }

        // Run the crawler with timeout if configured
        if self.max_run_time_seconds > 0 {
            // Use tokio::time::timeout but in a simpler way
            rt.block_on(async {
                let timeout_duration = std::time::Duration::from_secs(self.max_run_time_seconds);
                let timeout_future = tokio::time::sleep(timeout_duration);
                let run_future = async {
                    match engine.run().await {
                        Ok(_) => {}
                        Err(e) => warn!("Error running engine: {}", e),
                    }
                };

                tokio::select! {
                    _ = timeout_future => {
                        warn!("Crawler timed out after {} seconds", self.max_run_time_seconds);
                        info!("Performing controlled engine shutdown due to timeout");

                        // Use the new close() method to ensure proper cleanup
                        match engine.close().await {
                            Ok(_) => info!("Engine shutdown completed successfully after timeout"),
                            Err(e) => warn!("Error during engine shutdown after timeout: {}", e),
                        }
                    }
                    _ = run_future => {
                        info!("Crawler completed successfully within time limit");
                    }
                }
            });
        } else {
            // Run without timeout
            rt.block_on(async {
                match engine.run().await {
                    Ok(_) => {}
                    Err(e) => warn!("Error running engine: {}", e),
                }
            });
        }

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

        // Get stats from the engine instead of using hardcoded values
        let engine_stats = rt.block_on(async { engine.stats().await });
        result.request_count = engine_stats.request_count;
        result.response_count = engine_stats.response_count;
        result.item_count = engine_stats.item_count;
        result.error_count = engine_stats.error_count;

        // Calculate derived metrics
        result.calculate_metrics();

        // Save the result to a CSV file if an output directory is specified
        if let Some(dir) = &self.output_dir {
            let csv_path = format!("{}/{}_result.csv", dir, self.name);
            if let Err(e) = result.save_to_csv(&csv_path) {
                warn!("Failed to save benchmark result to CSV: {}", e);
            }
        }

        // Stop the mock server if it was started
        if let Some((mut server, runtime)) = mock_server {
            if let Some(pb) = &progress_bar {
                pb.set_message("Stopping mock server...");
            }

            runtime.block_on(async {
                server.stop().await;
            });

            if let Some(pb) = &progress_bar {
                pb.set_message("Mock server stopped");
            }
        }

        // Finish the progress bar if it exists
        if let Some(pb) = progress_bar {
            pb.finish_with_message("Benchmark completed");
        }

        result
    }
}
