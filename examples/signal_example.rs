use std::sync::Arc;

use log::{error, info};
use scrapy_rs_core::error::Result;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::response::Response;
use scrapy_rs_core::signal::{Signal, SignalArgs};
use scrapy_rs_core::spider::{BasicSpider, ParseOutput, Spider};
use scrapy_rs_engine::{Engine, EngineConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // Create a simple spider
    let spider = Arc::new(BasicSpider::new(
        "signal_example",
        vec![
            "http://localhost:3000/".to_string(),
            "http://localhost:3000/delay".to_string(),
        ],
    ));
    
    // Create engine
    let mut engine = Engine::new(spider.clone())?;
    
    // Get signal manager
    let signals = engine.signals();
    
    // Register signal handlers
    signals.connect(Signal::EngineStarted, |_| {
        info!("Engine started");
        Ok(())
    }).await?;
    
    signals.connect(Signal::SpiderOpened, |args| {
        if let SignalArgs::Spider(spider) = args {
            info!("Spider opened: {}", spider.name());
        }
        Ok(())
    }).await?;
    
    signals.connect(Signal::RequestScheduled, |args| {
        if let SignalArgs::Request(request) = args {
            info!("Request scheduled: {}", request.url);
        }
        Ok(())
    }).await?;
    
    signals.connect(Signal::RequestSent, |args| {
        if let SignalArgs::Request(request) = args {
            info!("Request sent: {}", request.url);
        }
        Ok(())
    }).await?;
    
    signals.connect(Signal::ResponseReceived, |args| {
        if let SignalArgs::Response(response) = args {
            info!("Response received: {} (status: {})", response.url, response.status);
        }
        Ok(())
    }).await?;
    
    signals.connect(Signal::ItemScraped, |args| {
        if let SignalArgs::Item(item) = args {
            info!("Item scraped: {:?}", item);
        }
        Ok(())
    }).await?;
    
    signals.connect(Signal::ErrorOccurred, |args| {
        if let SignalArgs::Error(error) = args {
            error!("Error occurred: {}", error);
        }
        Ok(())
    }).await?;
    
    signals.connect(Signal::SpiderClosed, |args| {
        if let SignalArgs::Spider(spider) = args {
            info!("Spider closed: {}", spider.name());
        }
        Ok(())
    }).await?;
    
    signals.connect(Signal::EngineStopped, |_| {
        info!("Engine stopped");
        Ok(())
    }).await?;
    
    // Configure engine
    let config = EngineConfig {
        concurrent_requests: 2,
        download_delay_ms: 1000,
        log_stats: true,
        ..EngineConfig::default()
    };
    engine = engine.with_config(config);
    
    // Run engine
    let stats = engine.run().await?;
    
    info!("Crawling completed, statistics: {:?}", stats);
    
    Ok(())
} 