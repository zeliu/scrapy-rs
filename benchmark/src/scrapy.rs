use std::env;
use std::fs::{self};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use chrono::Utc;
use indicatif::ProgressBar;
use log::{error, warn};
use serde_json::{self, Value};

use crate::common::TestScenario;
use crate::mock_server::{MockServer, MockServerConfig};
use crate::BenchmarkResult;
use crate::BenchmarkRunner;
use crate::{get_cpu_usage, get_memory_usage};

/// Benchmark runner for Scrapy
pub struct ScrapyBenchmarkRunner {
    /// Test scenario to run
    scenario: TestScenario,
    /// Name of the benchmark
    name: String,
    /// Output directory for results
    output_dir: Option<String>,
    /// Whether to show progress
    show_progress: bool,
    /// Path to the Python executable
    python_path: String,
    /// Path to the temporary directory for the benchmark
    temp_dir: PathBuf,
    /// Whether to use a mock server
    use_mock_server: bool,
    /// Mock server configuration
    mock_server_config: Option<MockServerConfig>,
}

impl ScrapyBenchmarkRunner {
    /// Create a new Scrapy benchmark runner
    pub fn new(scenario: TestScenario) -> Self {
        let temp_dir = env::temp_dir().join(format!("scrapy_benchmark_{}", scenario.name));

        Self {
            name: format!("{}_scrapy", scenario.name),
            scenario,
            output_dir: None,
            show_progress: true,
            python_path: "python".to_string(),
            temp_dir,
            use_mock_server: false,
            mock_server_config: None,
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

    /// Set the Python executable path
    pub fn with_python_path(mut self, path: &str) -> Self {
        self.python_path = path.to_string();
        self
    }

    /// Configure the benchmark to use a mock server
    pub fn with_mock_server(mut self, config: MockServerConfig) -> Self {
        self.use_mock_server = true;
        self.mock_server_config = Some(config);
        self
    }

    /// Copy template files to the temporary directory
    fn copy_template_files(&self) -> io::Result<()> {
        // Create necessary directories
        let project_dir = self.temp_dir.join("benchmark");
        let spiders_dir = project_dir.join("spiders");
        fs::create_dir_all(&spiders_dir)?;

        // Get template directory path
        let template_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("python_templates");

        // Copy files
        fs::copy(
            template_dir.join("__init__.py"),
            project_dir.join("__init__.py"),
        )?;
        fs::copy(template_dir.join("items.py"), project_dir.join("items.py"))?;
        fs::copy(
            template_dir.join("settings.py"),
            project_dir.join("settings.py"),
        )?;
        fs::copy(
            template_dir.join("pipelines.py"),
            project_dir.join("pipelines.py"),
        )?;

        // Copy spider files
        fs::copy(
            template_dir.join("spiders").join("__init__.py"),
            spiders_dir.join("__init__.py"),
        )?;
        fs::copy(
            template_dir.join("spiders").join("benchmark_spider.py"),
            spiders_dir.join("benchmark_spider.py"),
        )?;

        // Copy main file
        fs::copy(template_dir.join("main.py"), self.temp_dir.join("main.py"))?;

        Ok(())
    }

    /// Create a Scrapy project
    fn create_project(&self) -> io::Result<()> {
        // Create temporary directory
        fs::create_dir_all(&self.temp_dir)?;

        // Copy template files
        self.copy_template_files()?;

        // Modify settings file
        self.update_settings()?;

        // Modify spider file (don't set URLs yet if using mock server)
        if !self.use_mock_server {
            self.update_spider()?;
        } else {
            // Just update other aspects of the spider file, not the URLs
            let spider_path = self
                .temp_dir
                .join("benchmark")
                .join("spiders")
                .join("benchmark_spider.py");
            let content = fs::read_to_string(&spider_path)?;

            // Replace page limit and depth limit, but not start URLs
            let content = content.replace(
                "self.max_pages = 100",
                &format!("self.max_pages = {}", self.scenario.page_limit),
            );
            let content = content.replace(
                "self.max_depth = 2",
                &format!("self.max_depth = {}", self.scenario.max_depth),
            );

            // Write back file
            fs::write(spider_path, content)?;
        }

        // Modify pipeline file
        self.update_pipeline()?;

        Ok(())
    }

    /// Update the settings file
    fn update_settings(&self) -> io::Result<()> {
        let settings_path = self.temp_dir.join("benchmark").join("settings.py");
        let content = fs::read_to_string(&settings_path)?;

        // Replace settings
        let content = content.replace(
            "ROBOTSTXT_OBEY = True",
            &format!("ROBOTSTXT_OBEY = {}", self.scenario.respect_robots_txt),
        );
        let content = content.replace(
            "CONCURRENT_REQUESTS = 16",
            &format!(
                "CONCURRENT_REQUESTS = {}",
                self.scenario.concurrent_requests
            ),
        );
        let content = content.replace(
            "DOWNLOAD_DELAY = 0.0",
            &format!(
                "DOWNLOAD_DELAY = {}",
                self.scenario.download_delay_ms as f64 / 1000.0
            ),
        );
        let content = content.replace(
            "USER_AGENT = 'scrapy-benchmark/1.0'",
            &format!("USER_AGENT = '{}'", self.scenario.user_agent),
        );
        let content = content.replace(
            "REDIRECT_ENABLED = True",
            &format!("REDIRECT_ENABLED = {}", self.scenario.follow_redirects),
        );

        // Add other settings
        let mut additional_settings = String::new();
        for (key, value) in &self.scenario.settings {
            additional_settings.push_str(&format!("{} = '{}'\n", key, value));
        }
        let content = content.replace(
            "# other settings will be added at runtime",
            &additional_settings,
        );

        // Write back file
        fs::write(settings_path, content)?;

        Ok(())
    }

    /// Update the spider file
    fn update_spider(&self) -> io::Result<()> {
        let spider_path = self
            .temp_dir
            .join("benchmark")
            .join("spiders")
            .join("benchmark_spider.py");
        let content = fs::read_to_string(&spider_path)?;

        // Replace start URL, page limit, and depth limit
        let content = content.replace(
            "self.start_urls = []",
            &format!("self.start_urls = {}", self.format_urls()),
        );
        let content = content.replace(
            "self.max_pages = 100",
            &format!("self.max_pages = {}", self.scenario.page_limit),
        );
        let content = content.replace(
            "self.max_depth = 2",
            &format!("self.max_depth = {}", self.scenario.max_depth),
        );

        // Write back file
        fs::write(spider_path, content)?;

        Ok(())
    }

    /// Update the pipeline file
    fn update_pipeline(&self) -> io::Result<()> {
        let pipeline_path = self.temp_dir.join("benchmark").join("pipelines.py");
        let content = fs::read_to_string(&pipeline_path)?;

        // Replace output directory and file
        let mut updated_content = content.clone();

        if let Some(dir) = &self.output_dir {
            updated_content = updated_content.replace(
                "self.output_dir = None",
                &format!("self.output_dir = '{}'", dir),
            );
            let output_file_line = format!(
                "self.output_file = os.path.join(self.output_dir, 'items_{}.json')",
                self.name
            );
            updated_content = updated_content.replace("self.output_file = None", &output_file_line);
        }

        // Write back file
        fs::write(pipeline_path, updated_content)?;

        Ok(())
    }

    /// Format the URLs as a Python list
    fn format_urls(&self) -> String {
        let urls = self
            .scenario
            .urls
            .iter()
            .map(|url| format!("'{}'", url))
            .collect::<Vec<_>>()
            .join(", ");

        format!("[{}]", urls)
    }

    /// Run the spider and measure the performance
    fn run_spider(&self) -> io::Result<(BenchmarkResult, Vec<Value>)> {
        // Create a new benchmark result
        let mut result = BenchmarkResult::new(&self.name, "scrapy");
        result.start_time = Utc::now();

        // Create a progress bar if needed
        let progress_bar = if self.show_progress {
            Some(ProgressBar::new_spinner())
        } else {
            None
        };

        // Start the mock server if configured
        let mut mock_server = None;
        let mut urls_to_use = self.scenario.urls.clone();

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
                urls_to_use = vec![format!("{}/0", base_url)];

                mock_server = Some((server, runtime));

                if let Some(pb) = &progress_bar {
                    pb.set_message("Mock server started");
                }
            }
        }

        // Record memory and CPU usage before the benchmark
        let memory_before = get_memory_usage();
        let cpu_before = get_cpu_usage();

        // Run the spider and measure the time
        let start_time = Instant::now();

        // Create temporary spider file with the correct URLs
        let spider_path = self
            .temp_dir
            .join("benchmark")
            .join("spiders")
            .join("benchmark_spider.py");
        let spider_content = fs::read_to_string(&spider_path)?;

        // Format URLs for Python
        let formatted_urls = urls_to_use
            .iter()
            .map(|url| format!("'{}'", url))
            .collect::<Vec<_>>()
            .join(", ");
        let python_urls = format!("[{}]", formatted_urls);

        // Update spider file with the correct URLs
        let updated_spider_content = spider_content.replace(
            "self.start_urls = []",
            &format!("self.start_urls = {}", python_urls),
        );
        fs::write(&spider_path, updated_spider_content)?;

        // Create the stats file
        let stats_path = self.temp_dir.join("stats.json");

        // Run the spider using main.py
        let mut cmd = Command::new(&self.python_path);
        cmd.arg("main.py")
            .arg("--stats-file")
            .arg(stats_path.display().to_string())
            .current_dir(&self.temp_dir);

        // Add output directory if specified
        if let Some(dir) = &self.output_dir {
            cmd.arg("--output-dir").arg(dir);
        }

        // Execute the command
        let status = cmd.status()?;

        if !status.success() {
            // Stop the mock server if it was started
            if let Some((mut server, runtime)) = mock_server {
                runtime.block_on(async {
                    server.stop().await;
                });
            }

            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to run Scrapy spider.",
            ));
        }

        // Record the end time and duration
        let duration = start_time.elapsed();
        result.end_time = Utc::now();
        result.duration_ms = duration.as_millis() as u64;

        // Record memory and CPU usage after the benchmark
        let memory_after = get_memory_usage();
        let cpu_after = get_cpu_usage();

        // Calculate memory and CPU usage
        result.memory_usage_mb = memory_after - memory_before;
        result.cpu_usage_percent = cpu_after - cpu_before;

        // Read the stats file
        let stats_content = fs::read_to_string(stats_path)?;
        let stats: Value = serde_json::from_str(&stats_content)?;

        // Update the result with the stats
        if let Some(request_count) = stats["downloader/request_count"].as_u64() {
            result.request_count = request_count as usize;
        }

        if let Some(response_count) = stats["downloader/response_count"].as_u64() {
            result.response_count = response_count as usize;
        }

        if let Some(item_count) = stats["item_scraped_count"].as_u64() {
            result.item_count = item_count as usize;
        }

        if let Some(error_count) = stats["downloader/exception_count"].as_u64() {
            result.error_count = error_count as usize;
        }

        // Calculate derived metrics
        result.calculate_metrics();

        // Read the items file if it exists
        let items = if let Some(dir) = &self.output_dir {
            let items_path = Path::new(dir).join(format!("items_{}.json", self.name));
            if items_path.exists() {
                let items_content = fs::read_to_string(items_path)?;
                serde_json::from_str(&items_content)?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

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

        Ok((result, items))
    }
}

impl BenchmarkRunner for ScrapyBenchmarkRunner {
    fn name(&self) -> &str {
        &self.name
    }

    fn framework(&self) -> &str {
        "scrapy"
    }

    fn run(&mut self) -> BenchmarkResult {
        // Create the project
        if let Err(e) = self.create_project() {
            error!("Failed to create Scrapy project: {}", e);
            return BenchmarkResult::new(&self.name, "scrapy");
        }

        // Run the spider
        match self.run_spider() {
            Ok((result, _)) => result,
            Err(e) => {
                error!("Failed to run Scrapy spider: {}", e);
                BenchmarkResult::new(&self.name, "scrapy")
            }
        }
    }
}
