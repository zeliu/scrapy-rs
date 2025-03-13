use std::sync::Arc;
use std::time::Duration;

use log::{error, info};
use scrapy_rs_core::error::Result;
use scrapy_rs_core::spider::BasicSpider;
use scrapy_rs_engine::{Engine, EngineConfig};
use tokio::sync::Mutex;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Create a simple spider with local test URLs, add more URLs and longer delays
    let spider = Arc::new(BasicSpider::new(
        "pause_resume_example",
        vec![
            "http://localhost:3000/".to_string(),
            "http://localhost:3000/delay".to_string(), // This request will delay 5 seconds
            "http://localhost:3000/delay2".to_string(), // This request will delay 10 seconds
            "http://localhost:3000/delay3".to_string(), // This request will delay 15 seconds
            "http://localhost:3000/delay4".to_string(), // This request will delay 20 seconds
            "http://localhost:3000/delay5".to_string(), // This request will delay 25 seconds
            "http://localhost:3000/".to_string(),      // Repeat URL to increase crawling time
            "http://localhost:3000/delay".to_string(),
            "http://localhost:3000/delay2".to_string(),
            "http://localhost:3000/delay3".to_string(),
            "http://localhost:3000/delay4".to_string(),
            "http://localhost:3000/delay5".to_string(),
            "http://localhost:3000/".to_string(), // Repeat again
            "http://localhost:3000/delay".to_string(),
            "http://localhost:3000/delay2".to_string(),
        ],
    ));

    // Create engine
    let mut engine = Engine::new(spider.clone())?;

    // Configure engine with lower concurrency and longer download delay
    let config = EngineConfig {
        concurrent_requests: 1, // Set to 1 to make it easier to observe pause and resume
        download_delay_ms: 2000, // Increase download delay to 2 seconds
        log_stats: true,
        ..EngineConfig::default()
    };
    engine = engine.with_config(config);

    // Create a reference to the engine for use in the pause task
    let engine_ref = Arc::new(Mutex::new(engine));

    // Create a pause task
    let pause_task = {
        let engine_ref = engine_ref.clone();

        tokio::spawn(async move {
            // Wait 2 seconds before pausing the engine
            sleep(Duration::from_secs(2)).await;
            info!("Preparing to pause engine...");

            // Get a reference to the engine and pause it
            {
                let engine = engine_ref.lock().await;
                match engine.pause().await {
                    Ok(_) => {
                        info!("Engine paused");

                        // Save state
                        match engine.save_state("engine_state.json").await {
                            Ok(_) => info!("Engine state saved to engine_state.json"),
                            Err(e) => error!("Failed to save engine state: {}", e),
                        }
                    }
                    Err(e) => error!("Failed to pause engine: {}", e),
                }
            }

            // Wait 5 seconds before resuming the engine
            sleep(Duration::from_secs(5)).await;
            info!("Preparing to resume engine...");

            // Get a reference to the engine and resume it
            {
                let engine = engine_ref.lock().await;
                match engine.resume().await {
                    Ok(_) => info!("Engine resumed"),
                    Err(e) => error!("Failed to resume engine: {}", e),
                }
            }
        })
    };

    // Run the engine in the main thread
    let mut engine = engine_ref.lock().await;
    let stats = engine.run().await?;

    // Wait for the pause task to complete
    if let Err(e) = pause_task.await {
        error!("Pause task failed: {}", e);
    }

    info!("Crawling completed, statistics: {:?}", stats);

    // Try to load the saved state
    info!("Trying to load saved state...");
    match engine.load_state("engine_state.json").await {
        Ok(_) => {
            let loaded_stats = engine.stats().await;
            info!("Loaded statistics: {:?}", loaded_stats);
        }
        Err(e) => error!("Failed to load engine state: {}", e),
    }

    Ok(())
}
