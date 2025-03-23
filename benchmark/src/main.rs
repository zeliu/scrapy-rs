use std::fs;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use colored::*;
use env_logger::Env;
use log::{error, info, warn};

use scrapy_rs_benchmark::{
    common::{get_predefined_scenarios, TestScenario},
    compare_results, generate_report,
    mock_server::MockServerConfig,
    scrapy::ScrapyBenchmarkRunner,
    scrapy_rs::ScrapyRsBenchmarkRunner,
    BenchmarkResult, BenchmarkRunner,
};

/// Scrapy-RS Benchmark Tool
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Subcommand to run
    #[command(subcommand)]
    command: Commands,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Quiet output
    #[arg(short, long)]
    quiet: bool,
}

/// Benchmark commands
#[derive(Subcommand)]
enum Commands {
    /// Run a benchmark
    Run(Box<RunArgs>),

    /// List available scenarios
    List,

    /// Generate a report from existing results
    Report(ReportArgs),
}

/// Arguments for the run command
#[derive(Args)]
struct RunArgs {
    /// Scenario to run
    #[arg(short, long)]
    scenario: Option<String>,

    /// Run only Scrapy-RS
    #[arg(long)]
    only_scrapy_rs: bool,

    /// Run only Scrapy
    #[arg(long)]
    only_scrapy: bool,

    /// Python executable path
    #[arg(long, default_value = "python")]
    python: String,

    /// Custom URLs to crawl
    #[arg(short, long)]
    urls: Option<Vec<String>>,

    /// Page limit
    #[arg(long)]
    page_limit: Option<usize>,

    /// Maximum depth
    #[arg(long)]
    max_depth: Option<usize>,

    /// Concurrent requests
    #[arg(long)]
    concurrent_requests: Option<usize>,

    /// Download delay in milliseconds
    #[arg(long)]
    download_delay: Option<u64>,

    /// User agent
    #[arg(long)]
    user_agent: Option<String>,

    /// Follow redirects
    #[arg(long)]
    follow_redirects: Option<bool>,

    /// Respect robots.txt
    #[arg(long)]
    respect_robots_txt: Option<bool>,

    /// Output directory for results
    #[arg(short, long, value_name = "DIR")]
    output_dir: Option<PathBuf>,

    /// Use a mock server instead of real URLs
    #[arg(long)]
    use_mock_server: bool,

    /// Port for the mock server
    #[arg(long, default_value = "8000")]
    mock_server_port: u16,

    /// Number of pages for the mock server
    #[arg(long, default_value = "100")]
    mock_server_pages: usize,

    /// Links per page for the mock server
    #[arg(long, default_value = "10")]
    mock_server_links: usize,

    /// Response delay in milliseconds for the mock server
    #[arg(long, default_value = "0")]
    mock_server_delay: u64,

    /// Simulate failures in the mock server
    #[arg(long)]
    mock_server_failures: bool,

    /// Failure rate (0.0 - 1.0) for the mock server
    #[arg(long, default_value = "0.1")]
    mock_server_failure_rate: f64,

    /// Maximum run time in seconds (0 means no limit)
    #[arg(long, default_value = "30")]
    max_run_time_seconds: u64,
}

/// Arguments for the report command
#[derive(Args)]
struct ReportArgs {
    /// Input directory with results
    #[arg(short, long, value_name = "DIR")]
    input_dir: PathBuf,
}

fn main() {
    // Initialize logger
    let env = Env::default().filter_or("RUST_LOG", "info");
    env_logger::init_from_env(env);

    // Parse command line arguments
    let cli = Cli::parse();

    // Set log level based on verbosity
    if cli.verbose {
        log::set_max_level(log::LevelFilter::Debug);
    } else if cli.quiet {
        log::set_max_level(log::LevelFilter::Warn);
    }

    // Process command
    match cli.command {
        Commands::Run(args) => {
            run_benchmark(args, None);
        }
        Commands::List => {
            list_scenarios();
        }
        Commands::Report(args) => {
            generate_benchmark_report(args, None);
        }
    }
}

/// Run a benchmark
fn run_benchmark(args: Box<RunArgs>, output_dir: Option<String>) {
    // Get the scenario
    let scenario = if let Some(name) = args.scenario {
        // Find the predefined scenario
        let scenarios = get_predefined_scenarios();
        let scenario = scenarios.iter().find(|s| s.name == name);

        match scenario {
            Some(s) => s.clone(),
            None => {
                error!(
                    "Scenario '{}' not found. Use 'list' command to see available scenarios.",
                    name
                );
                return;
            }
        }
    } else {
        // Create a custom scenario
        let mut scenario = TestScenario::default();

        if let Some(urls) = args.urls {
            scenario = scenario.with_urls(urls);
        }

        if let Some(limit) = args.page_limit {
            scenario = scenario.with_page_limit(limit);
        }

        if let Some(depth) = args.max_depth {
            scenario = scenario.with_max_depth(depth);
        }

        if let Some(requests) = args.concurrent_requests {
            scenario = scenario.with_concurrent_requests(requests);
        }

        if let Some(delay) = args.download_delay {
            scenario = scenario.with_download_delay(delay);
        }

        if let Some(user_agent) = args.user_agent {
            scenario = scenario.with_user_agent(&user_agent);
        }

        if let Some(follow) = args.follow_redirects {
            scenario = scenario.with_follow_redirects(follow);
        }

        if let Some(respect) = args.respect_robots_txt {
            scenario = scenario.with_respect_robots_txt(respect);
        }

        scenario
    };

    // Create output directory if specified
    let output_dir = args
        .output_dir
        .map(|p| p.to_string_lossy().to_string())
        .or(output_dir);

    if let Some(ref dir) = output_dir {
        if let Err(e) = fs::create_dir_all(dir) {
            error!("Failed to create output directory: {}", e);
            return;
        }
    }

    // Configure the mock server if requested
    let mock_server_config = if args.use_mock_server {
        info!("Mock server will be used for benchmarking");
        Some(
            MockServerConfig::new()
                .with_port(args.mock_server_port)
                .with_page_count(args.mock_server_pages)
                .with_links_per_page(args.mock_server_links)
                .with_response_delay(args.mock_server_delay)
                .with_simulate_failures(args.mock_server_failures)
                .with_failure_rate(args.mock_server_failure_rate),
        )
    } else {
        None
    };

    // Print benchmark information
    info!("Running benchmark: {}", scenario.name.bold());
    info!("Description: {}", scenario.description);
    if args.use_mock_server {
        info!("{}", "USING MOCK SERVER".bold().green());
        info!("Mock server port: {}", args.mock_server_port);
        info!("Mock server pages: {}", args.mock_server_pages);
        info!("Mock server links per page: {}", args.mock_server_links);
        info!("Mock server response delay: {}ms", args.mock_server_delay);
        if args.mock_server_failures {
            info!(
                "Mock server failures enabled with rate: {:.2}",
                args.mock_server_failure_rate
            );
        }
        info!("Original URLs will be ignored");
    } else {
        info!("URLs: {}", scenario.urls.join(", "));
    }
    info!("Page limit: {}", scenario.page_limit);
    info!("Max depth: {}", scenario.max_depth);
    info!("Concurrent requests: {}", scenario.concurrent_requests);
    info!("Download delay: {}ms", scenario.download_delay_ms);

    // Run the benchmarks
    let mut scrapy_result = None;
    let mut scrapy_rs_result = None;

    // Run Scrapy benchmark if requested
    if !args.only_scrapy_rs {
        info!("Running Scrapy benchmark...");

        let mut runner =
            ScrapyBenchmarkRunner::new(scenario.clone()).with_python_path(&args.python);

        if let Some(ref dir) = output_dir {
            runner = runner.with_output_dir(dir);
        }

        // Configure mock server if requested
        if args.use_mock_server {
            if let Some(config) = &mock_server_config {
                runner = runner.with_mock_server(config.clone());
            }
        }

        let result = runner.run();

        info!(
            "Scrapy benchmark completed in {:.2} seconds",
            result.duration_ms as f64 / 1000.0
        );
        info!(
            "Requests: {}, Responses: {}, Items: {}, Errors: {}",
            result.request_count, result.response_count, result.item_count, result.error_count
        );
        info!("Requests per second: {:.2}", result.requests_per_second);

        scrapy_result = Some(result);
    }

    // Run Scrapy-RS benchmark if requested
    if !args.only_scrapy {
        info!("Running Scrapy-RS benchmark...");

        let mut runner = ScrapyRsBenchmarkRunner::new(scenario.clone());

        if let Some(ref dir) = output_dir {
            runner = runner.with_output_dir(dir);
        }

        // Configure mock server if requested
        if args.use_mock_server {
            if let Some(config) = &mock_server_config {
                runner = runner.with_mock_server(config.clone());
            }
        }

        // Set maximum run time
        runner = runner.with_max_run_time(args.max_run_time_seconds);

        let result = runner.run();

        info!(
            "Scrapy-RS benchmark completed in {:.2} seconds",
            result.duration_ms as f64 / 1000.0
        );
        info!(
            "Requests: {}, Responses: {}, Items: {}, Errors: {}",
            result.request_count, result.response_count, result.item_count, result.error_count
        );
        info!("Requests per second: {:.2}", result.requests_per_second);

        scrapy_rs_result = Some(result);
    }

    // Compare results if both benchmarks were run
    if let (Some(scrapy), Some(scrapy_rs)) = (&scrapy_result, &scrapy_rs_result) {
        compare_results(scrapy, scrapy_rs);

        // Generate report if output directory is specified
        if let Some(ref dir) = output_dir {
            let results = vec![scrapy.clone(), scrapy_rs.clone()];
            if let Err(e) = generate_report(&results, dir) {
                error!("Failed to generate report: {}", e);
            } else {
                info!("Report generated at {}/benchmark_report.html", dir);
            }
        }
    }
}

/// List available scenarios
fn list_scenarios() {
    let scenarios = get_predefined_scenarios();

    println!("{}", "Available benchmark scenarios:".bold());
    println!();

    for scenario in scenarios {
        println!("{}: {}", scenario.name.bold(), scenario.description);
        println!("  URLs: {}", scenario.urls.join(", "));
        println!("  Page limit: {}", scenario.page_limit);
        println!("  Max depth: {}", scenario.max_depth);
        println!("  Concurrent requests: {}", scenario.concurrent_requests);
        println!("  Download delay: {}ms", scenario.download_delay_ms);
        println!();
    }
}

/// Generate a benchmark report
fn generate_benchmark_report(args: ReportArgs, output_dir: Option<String>) {
    // Determine output directory
    let out_dir = output_dir.unwrap_or_else(|| "benchmark_reports".to_string());

    // Create output directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&out_dir) {
        error!("Failed to create output directory: {}", e);
        return;
    }

    // Read all CSV files in the input directory
    let entries = match fs::read_dir(&args.input_dir) {
        Ok(entries) => entries,
        Err(e) => {
            error!("Failed to read input directory: {}", e);
            return;
        }
    };

    // Collect results
    let mut results = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                warn!("Failed to read directory entry: {}", e);
                continue;
            }
        };

        let path = entry.path();

        // Skip non-CSV files
        if path.extension().is_some_and(|ext| ext == "csv") {
            // Read the CSV file
            let file = match fs::File::open(&path) {
                Ok(file) => file,
                Err(e) => {
                    warn!("Failed to open file {}: {}", path.display(), e);
                    continue;
                }
            };

            // Parse the CSV file
            let mut rdr = csv::Reader::from_reader(file);
            for result in rdr.deserialize() {
                let result: BenchmarkResult = match result {
                    Ok(result) => result,
                    Err(e) => {
                        warn!("Failed to parse CSV record: {}", e);
                        continue;
                    }
                };

                results.push(result);
            }
        }
    }

    // Generate the report
    if results.is_empty() {
        error!("No benchmark results found in {}", args.input_dir.display());
        return;
    }

    if let Err(e) = generate_report(&results, &out_dir) {
        error!("Failed to generate report: {}", e);
    } else {
        info!("Report generated at {}/benchmark_report.html", out_dir);
    }
}
