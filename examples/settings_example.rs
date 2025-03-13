use scrapy_rs::config_adapters::{
    create_downloader_from_settings, create_scheduler_from_settings, create_spider_from_settings,
    downloader_config_from_settings, engine_config_from_settings,
};
use scrapy_rs::prelude::*;
use scrapy_rs::settings::Settings;
use scrapy_rs_engine::Engine;
use scrapy_rs_pipeline::LogPipeline;
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::init();

    // Load settings from file
    println!("Loading settings from file...");
    let settings = Settings::from_file("examples/settings.py")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Print all settings
    println!("\nAll settings:");
    for (key, value) in settings.all() {
        println!("  {}: {}", key, value);
    }

    // Get specific settings
    let bot_name: String = settings
        .get("BOT_NAME")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let user_agent: String = settings
        .get("USER_AGENT")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let concurrent_requests: usize = settings
        .get("CONCURRENT_REQUESTS")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let download_delay_ms: u64 = settings
        .get("DOWNLOAD_DELAY_MS")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let follow_redirects: bool = settings
        .get("FOLLOW_REDIRECTS")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let start_urls: Vec<String> = settings
        .get("START_URLS")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let allowed_domains: Vec<String> = settings
        .get("ALLOWED_DOMAINS")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!("\nSpecific settings:");
    println!("  Bot name: {}", bot_name);
    println!("  User agent: {}", user_agent);
    println!("  Concurrent requests: {}", concurrent_requests);
    println!("  Download delay (ms): {}", download_delay_ms);
    println!("  Follow redirects: {}", follow_redirects);
    println!("  Start URLs: {:?}", start_urls);
    println!("  Allowed domains: {:?}", allowed_domains);

    // Create components from settings using adapter functions
    println!("\nCreating components using adapter functions:");

    // Create EngineConfig
    let engine_config = engine_config_from_settings(&settings)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    println!(
        "  Created EngineConfig: concurrent_requests={}",
        engine_config.concurrent_requests
    );

    // Create DownloaderConfig
    let downloader_config = downloader_config_from_settings(&settings)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    println!(
        "  Created DownloaderConfig: timeout={}s",
        downloader_config.timeout
    );

    // Create Spider
    let spider = create_spider_from_settings(&settings)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    println!("  Created Spider: name={}", spider.name());

    // Create Downloader
    let downloader = create_downloader_from_settings(&settings)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    println!("  Created Downloader");

    // Create Scheduler
    let scheduler = create_scheduler_from_settings(&settings)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    println!("  Created Scheduler");

    // Create Engine
    let _engine = Engine::with_components(
        spider.clone(),
        scheduler,
        downloader,
        Arc::new(LogPipeline::info()),
        Arc::new(DefaultHeadersMiddleware::common()),
        Arc::new(ResponseLoggerMiddleware::info()),
        engine_config,
    );

    println!("\nEngine created with settings from file.");
    println!("In a real application, you would now run the engine with engine.run().await");

    Ok(())
}
