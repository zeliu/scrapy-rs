use clap::{Parser, Subcommand};
use scrapy_rs::config_adapters::*;
use std::env;
use std::fs;
use std::path::Path;
use std::process;
use std::sync::Arc;

use scrapy_rs::downloader::{DownloaderConfig, HttpDownloader};
use scrapy_rs::engine::Engine;
use scrapy_rs::middleware::{DefaultHeadersMiddleware, ResponseLoggerMiddleware};
use scrapy_rs::pipeline::LogPipeline;
use scrapy_rs::settings::Settings;
use scrapy_rs_core::spider::BasicSpider;
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(
    name = "scrapyrs",
    about = "A high-performance web crawler written in Rust",
    version,
    author,
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Scrapy-RS project
    #[command(name = "startproject")]
    StartProject {
        /// Name of the project
        name: String,

        /// Directory to create the project in (defaults to the project name)
        #[arg(long)]
        directory: Option<String>,
    },

    /// Generate a new spider
    #[command(name = "genspider")]
    GenSpider {
        /// Name of the spider
        name: String,

        /// Domain to crawl
        domain: String,

        /// Template to use (basic or custom)
        #[arg(long, default_value = "basic")]
        template: String,
    },

    /// Run a spider
    #[command(name = "crawl")]
    Crawl {
        /// Name of the spider to run
        name: String,

        /// Output file for scraped items
        #[arg(short, long)]
        output: Option<String>,

        /// Format of the output file (json, csv)
        #[arg(long, default_value = "json")]
        format: String,

        /// Settings file to use
        #[arg(short, long)]
        settings: Option<String>,
    },

    /// List available spiders
    #[command(name = "list")]
    List {
        /// Settings file to use
        #[arg(short, long)]
        settings: Option<String>,
    },

    /// Run a spider defined in a file
    #[command(name = "runspider")]
    RunSpider {
        /// Path to the spider file
        file: String,

        /// Output file for scraped items
        #[arg(short, long)]
        output: Option<String>,

        /// Format of the output file (json, csv)
        #[arg(long, default_value = "json")]
        format: String,

        /// Settings file to use
        #[arg(short, long)]
        settings: Option<String>,
    },

    /// Get or set a setting value
    #[command(name = "settings")]
    Settings {
        /// Name of the setting to get or set
        name: Option<String>,

        /// Value to set the setting to
        value: Option<String>,

        /// Settings file to use
        #[arg(short, long)]
        settings_file: Option<String>,
    },

    /// Show version information
    #[command(name = "version")]
    Version,
}

fn main() {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize logger
    env_logger::init();

    // Execute the appropriate command
    match cli.command {
        Commands::StartProject { name, directory } => {
            start_project(&name, directory.as_deref());
        }
        Commands::GenSpider {
            name,
            domain,
            template,
        } => {
            gen_spider(&name, &domain, &template);
        }
        Commands::Crawl {
            name,
            output,
            format,
            settings,
        } => {
            crawl(&name, output.as_deref(), &format, settings.as_deref());
        }
        Commands::List { settings } => {
            list_spiders(settings.as_deref());
        }
        Commands::RunSpider {
            file,
            output,
            format,
            settings,
        } => {
            run_spider(&file, output.as_deref(), &format, settings.as_deref());
        }
        Commands::Settings {
            name,
            value,
            settings_file,
        } => {
            manage_settings(name.as_deref(), value.as_deref(), settings_file.as_deref());
        }
        Commands::Version => {
            show_version();
        }
    }
}

fn start_project(name: &str, directory: Option<&str>) {
    let dir = directory.unwrap_or(name);
    let project_dir = Path::new(dir).join(name);

    println!(
        "Creating project '{}' in directory '{}'",
        name,
        project_dir.display()
    );

    // Create project directory
    if let Err(e) = fs::create_dir_all(&project_dir) {
        eprintln!("Error creating project directory: {}", e);
        process::exit(1);
    }

    // Create settings.py
    let settings_path = project_dir.join("settings.py");
    let settings_content = format!(
        r#"# Scrapy-RS settings file
# This file contains the configuration for the Scrapy-RS crawler

# Basic settings
BOT_NAME = '{}'
USER_AGENT = 'scrapy_rs/0.1.0 (+https://github.com/liuze/scrapy_rs)'

# Crawl settings
CONCURRENT_REQUESTS = 16
DOWNLOAD_DELAY_MS = 0
REQUEST_TIMEOUT = 30
FOLLOW_REDIRECTS = True
MAX_RETRIES = 3
RETRY_ENABLED = True
RESPECT_ROBOTS_TXT = True

# Limits
MAX_DEPTH = None
MAX_REQUESTS_PER_DOMAIN = None
MAX_REQUESTS_PER_SPIDER = None

# Logging
LOG_LEVEL = 'INFO'
LOG_REQUESTS = True
LOG_ITEMS = True
LOG_STATS = True
STATS_INTERVAL_SECS = 60

# Pipelines
ITEM_PIPELINES = [
    'LogPipeline',
    'JsonFilePipeline',
]

# Pipeline settings
JSON_FILE_PATH = 'items.json'

# Middleware
REQUEST_MIDDLEWARES = [
    'DefaultHeadersMiddleware',
    'RandomDelayMiddleware',
]

RESPONSE_MIDDLEWARES = [
    'ResponseLoggerMiddleware',
    'RetryMiddleware',
]

# Spider settings
ALLOWED_DOMAINS = []
START_URLS = []
"#,
        name
    );

    if let Err(e) = fs::write(&settings_path, settings_content) {
        eprintln!("Error creating settings.py: {}", e);
        process::exit(1);
    }

    // Create spiders directory
    let spiders_dir = project_dir.join("spiders");
    if let Err(e) = fs::create_dir_all(&spiders_dir) {
        eprintln!("Error creating spiders directory: {}", e);
        process::exit(1);
    }

    // Create __init__.py in spiders directory
    let init_path = spiders_dir.join("__init__.py");
    if let Err(e) = fs::write(
        &init_path,
        "# This file is required to make Python treat the directory as a package\n",
    ) {
        eprintln!("Error creating __init__.py: {}", e);
        process::exit(1);
    }

    // Create items.py
    let items_path = project_dir.join("items.py");
    let items_content = r#"# Define your item models here
#
# Example:
# class ExampleItem:
#     def __init__(self):
#         self.name = ""
#         self.description = ""
#         self.price = 0.0
"#;

    if let Err(e) = fs::write(&items_path, items_content) {
        eprintln!("Error creating items.py: {}", e);
        process::exit(1);
    }

    // Create pipelines.py
    let pipelines_path = project_dir.join("pipelines.py");
    let pipelines_content = r#"# Define your item pipelines here
#
# Example:
# class ExamplePipeline:
#     def process_item(self, item, spider):
#         # Process the item here
#         return item
"#;

    if let Err(e) = fs::write(&pipelines_path, pipelines_content) {
        eprintln!("Error creating pipelines.py: {}", e);
        process::exit(1);
    }

    // Create middlewares.py
    let middlewares_path = project_dir.join("middlewares.py");
    let middlewares_content = r#"# Define your middlewares here
#
# Example:
# class ExampleMiddleware:
#     def process_request(self, request, spider):
#         # Process the request here
#         return None
#
#     def process_response(self, request, response, spider):
#         # Process the response here
#         return response
"#;

    if let Err(e) = fs::write(&middlewares_path, middlewares_content) {
        eprintln!("Error creating middlewares.py: {}", e);
        process::exit(1);
    }

    println!("Project '{}' created successfully", name);
    println!(
        "You can now cd into '{}' and start creating your spiders",
        project_dir.display()
    );
}

fn gen_spider(name: &str, domain: &str, template: &str) {
    // Check if spiders directory exists
    let spiders_dir = Path::new("spiders");
    if !spiders_dir.exists() {
        eprintln!("Error: spiders directory not found. Make sure you are in a Scrapy-RS project directory.");
        process::exit(1);
    }

    // Create spider file
    let spider_path = spiders_dir.join(format!("{}.py", name));

    // Check if spider already exists
    if spider_path.exists() {
        eprintln!("Error: spider '{}' already exists", name);
        process::exit(1);
    }

    // Generate spider content based on template
    let spider_content = match template {
        "basic" => {
            format!(
                r#"from scrapy-rs import PySpider, PyItem, PyRequest

class {}Spider:
    """
    A spider that crawls {}
    """
    
    def __init__(self):
        self.spider = PySpider(
            name="{}",
            start_urls=["https://{}"],
            allowed_domains=["{}"]
        )
    
    def parse(self, response):
        """Parse a response and extract data."""
        print(f"Parsing {{response.url}} (status: {{response.status}})")
        
        # Create an item to store the data
        item = PyItem("webpage")
        item.set("url", response.url)
        
        # Extract title from the response
        # In a real spider, you would use a library like BeautifulSoup to parse the HTML
        
        # Return the item and any new requests to follow
        return [item], []
"#,
                name, domain, name, domain, domain
            )
        }
        "custom" => {
            format!(
                r#"from scrapy-rs import PySpider, PyItem, PyRequest

class {}Spider:
    """
    A custom spider that crawls {}
    """
    
    def __init__(self):
        self.spider = PySpider(
            name="{}",
            start_urls=["https://{}"],
            allowed_domains=["{}"]
        )
        self.visited_urls = set()
    
    def parse(self, response):
        """Parse a response and extract data."""
        self.visited_urls.add(response.url)
        print(f"Parsing {{response.url}} (status: {{response.status}})")
        
        # Create items to store the data
        items = []
        
        # Extract data from the response
        # In a real spider, you would use a library like BeautifulSoup to parse the HTML
        item = PyItem("webpage")
        item.set("url", response.url)
        items.append(item)
        
        # Extract links from the response and create new requests
        new_requests = []
        
        # In a real spider, you would extract links from the HTML
        # For example:
        # for link in response.extract_links():
        #     if link not in self.visited_urls:
        #         request = PyRequest(link)
        #         new_requests.append(request)
        
        return items, new_requests
"#,
                name, domain, name, domain, domain
            )
        }
        _ => {
            eprintln!("Error: unknown template '{}'", template);
            process::exit(1);
        }
    };

    // Write spider file
    if let Err(e) = fs::write(&spider_path, spider_content) {
        eprintln!("Error creating spider file: {}", e);
        process::exit(1);
    }

    println!("Spider '{}' created successfully", name);
    println!("You can now edit '{}'", spider_path.display());
}

fn crawl(name: &str, output: Option<&str>, format: &str, settings_path: Option<&str>) {
    println!("Running spider '{}'", name);

    // Load settings
    let _settings = load_settings(settings_path);

    // Check if spiders directory exists
    let spiders_dir = Path::new("spiders");
    if !spiders_dir.exists() {
        eprintln!("Error: spiders directory not found. Make sure you are in a Scrapy-RS project directory.");
        process::exit(1);
    }

    // Find the spider file
    let spider_file = spiders_dir.join(format!("{}.py", name));
    if !spider_file.exists() {
        eprintln!("Error: spider '{}' not found.", name);
        process::exit(1);
    }

    // Run the spider
    run_spider_file(&spider_file, output, format, settings_path);
}

fn list_spiders(settings_path: Option<&str>) {
    // Load settings
    let _settings = load_settings(settings_path);

    // Check if spiders directory exists
    let spiders_dir = Path::new("spiders");
    if !spiders_dir.exists() {
        eprintln!("Error: spiders directory not found. Make sure you are in a Scrapy-RS project directory.");
        process::exit(1);
    }

    // List all Python files in the spiders directory
    println!("Available spiders:");

    let entries = match fs::read_dir(spiders_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading spiders directory: {}", e);
            process::exit(1);
        }
    };

    let mut found_spiders = false;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "py") {
            if let Some(stem) = path.file_stem() {
                if let Some(name) = stem.to_str() {
                    if name != "__init__" {
                        println!("  {}", name);
                        found_spiders = true;
                    }
                }
            }
        }
    }

    if !found_spiders {
        println!("  No spiders found");
    }
}

fn run_spider(file: &str, output: Option<&str>, format: &str, settings_path: Option<&str>) {
    println!("Running spider from file '{}'", file);

    // Load settings
    let _settings = load_settings(settings_path);

    // Check if the file exists
    let spider_file = Path::new(file);
    if !spider_file.exists() {
        eprintln!("Error: spider file '{}' not found.", file);
        process::exit(1);
    }

    // Run the spider
    run_spider_file(spider_file, output, format, settings_path);
}

/// Run a spider from a file
fn run_spider_file(
    spider_file: &Path,
    output: Option<&str>,
    _format: &str,
    settings_path: Option<&str>,
) {
    // Read the spider file
    let _spider_code = match fs::read_to_string(spider_file) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error reading spider file: {}", e);
            process::exit(1);
        }
    };

    // Get the spider name from the file name
    let spider_name = match spider_file.file_stem() {
        Some(name) => name.to_string_lossy().to_string(),
        None => {
            eprintln!("Error: invalid spider file name.");
            process::exit(1);
        }
    };

    // Create a runtime for async operations
    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Error creating runtime: {}", e);
            process::exit(1);
        }
    };

    // Load settings
    let settings = load_settings(settings_path);

    println!("Loading spider from file: {}", spider_file.display());
    println!("Spider name: {}", spider_name);

    // Create a basic spider with settings from the file
    #[allow(unused_assignments)]
    let mut spider = BasicSpider::new(&spider_name, vec![]);

    // Add start URLs from settings
    if let Ok(start_urls) = settings.get::<Vec<String>>("START_URLS") {
        if !start_urls.is_empty() {
            println!("Using start URLs from settings: {:?}", start_urls);
            spider = BasicSpider::new(&spider_name, start_urls);
        } else {
            // Use default start URL
            let start_urls = vec!["https://example.com".to_string()];
            println!("Using default start URL: {:?}", start_urls);
            spider = BasicSpider::new(&spider_name, start_urls);
        }
    } else {
        // Use default start URL
        let start_urls = vec!["https://example.com".to_string()];
        println!("Using default start URL: {:?}", start_urls);
        spider = BasicSpider::new(&spider_name, start_urls);
    }

    // Add allowed domains from settings
    if let Ok(allowed_domains) = settings.get::<Vec<String>>("ALLOWED_DOMAINS") {
        if !allowed_domains.is_empty() {
            println!("Using allowed domains from settings: {:?}", allowed_domains);
            spider = spider.with_allowed_domains(allowed_domains);
        } else {
            // Use default allowed domain
            let allowed_domains = vec!["example.com".to_string()];
            println!("Using default allowed domain: {:?}", allowed_domains);
            spider = spider.with_allowed_domains(allowed_domains);
        }
    } else {
        // Use default allowed domain
        let allowed_domains = vec!["example.com".to_string()];
        println!("Using default allowed domain: {:?}", allowed_domains);
        spider = spider.with_allowed_domains(allowed_domains);
    }

    // Create downloader configuration
    let mut downloader_config = DownloaderConfig::default();

    // Set user agent from settings
    if let Ok(user_agent) = settings.get::<String>("USER_AGENT") {
        println!("Using user agent from settings: {}", user_agent);
        downloader_config.user_agent = user_agent;
    } else {
        // Use default user agent
        let user_agent = "scrapy_rs/0.1.0".to_string();
        println!("Using default user agent: {}", user_agent);
        downloader_config.user_agent = user_agent;
    }

    // Set timeout from settings
    if let Ok(timeout) = settings.get::<u64>("REQUEST_TIMEOUT") {
        println!("Using timeout from settings: {} seconds", timeout);
        downloader_config.timeout = timeout;
    } else {
        // Use default timeout
        let timeout = 30;
        println!("Using default timeout: {} seconds", timeout);
        downloader_config.timeout = timeout;
    }

    // Set concurrent requests from settings
    if let Ok(concurrent_requests) = settings.get::<usize>("CONCURRENT_REQUESTS") {
        println!(
            "Using concurrent requests from settings: {}",
            concurrent_requests
        );
        downloader_config.concurrent_requests = concurrent_requests;
    } else {
        // Use default concurrent requests
        let concurrent_requests = 16;
        println!("Using default concurrent requests: {}", concurrent_requests);
        downloader_config.concurrent_requests = concurrent_requests;
    }

    // Create components
    let downloader = match HttpDownloader::new(downloader_config) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error creating downloader: {}", e);
            process::exit(1);
        }
    };

    // Create scheduler using the adapter function
    let scheduler = match create_scheduler_from_settings(&settings) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error creating scheduler: {}", e);
            process::exit(1);
        }
    };

    let pipeline = Arc::new(LogPipeline::info());
    let request_middleware = Arc::new(DefaultHeadersMiddleware::common());
    let response_middleware = Arc::new(ResponseLoggerMiddleware::info());

    // Create engine configuration from settings
    let engine_config = match engine_config_from_settings(&settings) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error creating engine configuration: {}", e);
            process::exit(1);
        }
    };

    println!(
        "Creating engine with scheduler type: {:?}, strategy: {:?}",
        engine_config.scheduler_type, engine_config.crawl_strategy
    );

    // Create engine
    let mut engine = runtime.block_on(async {
        Engine::with_components(
            Arc::new(spider),
            scheduler,
            Arc::new(downloader),
            pipeline,
            request_middleware,
            response_middleware,
            engine_config,
        )
    });

    println!("Running engine...");

    // Run the engine
    let stats = runtime.block_on(async { engine.run().await });

    // Handle the Result<EngineStats, Error>
    match stats {
        Ok(stats) => {
            // Print statistics
            println!("\nCrawl completed!");
            println!("Requests: {}", stats.request_count);
            println!("Responses: {}", stats.response_count);
            println!("Items: {}", stats.item_count);
            println!("Errors: {}", stats.error_count);
            if let Some(duration) = stats.duration() {
                println!("Duration: {:.8} seconds", duration.as_secs_f64());
            }
            if let Some(rps) = stats.requests_per_second() {
                println!("Requests per second: {:.2}", rps);
            }
        }
        Err(e) => {
            eprintln!("Error running engine: {}", e);
            process::exit(1);
        }
    }

    // Save output if requested
    if let Some(output_path) = output {
        println!("Saving output to {}", output_path);
        // TODO: Implement output saving
    }
}

fn manage_settings(name: Option<&str>, value: Option<&str>, settings_file: Option<&str>) {
    // Load settings
    let mut settings = load_settings(settings_file);

    match (name, value) {
        (Some(name), Some(value)) => {
            // Set the setting
            println!("Setting {} = {}", name, value);

            // Parse the value based on its content
            let parsed_value = if value == "True" || value == "true" {
                serde_json::Value::Bool(true)
            } else if value == "False" || value == "false" {
                serde_json::Value::Bool(false)
            } else if value == "None" || value == "null" {
                serde_json::Value::Null
            } else if let Ok(num) = value.parse::<i64>() {
                serde_json::Value::Number(num.into())
            } else if let Ok(num) = value.parse::<f64>() {
                if let Some(num) = serde_json::Number::from_f64(num) {
                    serde_json::Value::Number(num)
                } else {
                    serde_json::Value::String(value.to_string())
                }
            } else {
                serde_json::Value::String(value.to_string())
            };

            if let Err(e) = settings.set(name, parsed_value) {
                eprintln!("Error setting setting: {}", e);
                process::exit(1);
            }

            // Save the settings
            let settings_path = settings_file.unwrap_or("settings.py");
            if let Err(e) = settings.save(settings_path) {
                eprintln!("Error saving settings: {}", e);
                process::exit(1);
            }

            println!("Setting saved successfully");
        }
        (Some(name), None) => {
            // Get the setting
            match settings.get::<serde_json::Value>(name) {
                Ok(value) => {
                    println!("{} = {}", name, value);
                }
                Err(e) => {
                    eprintln!("Error getting setting: {}", e);
                    process::exit(1);
                }
            }
        }
        (None, _) => {
            // List all settings
            println!("All settings:");
            for (key, value) in settings.all() {
                println!("  {} = {}", key, value);
            }
        }
    }
}

fn show_version() {
    println!("Scrapy-RS version {}", env!("CARGO_PKG_VERSION"));
    println!("A high-performance web crawler written in Rust");
}

fn load_settings(settings_path: Option<&str>) -> Settings {
    let settings_path = settings_path.unwrap_or("settings.py");

    match Settings::from_file(settings_path) {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Error loading settings from {}: {}", settings_path, e);
            eprintln!("Using default settings");
            Settings::default()
        }
    }
}
