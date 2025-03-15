use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::StreamExt;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio::time::interval;

use scrapy_rs_core::error::{Error, Result as CoreResult};
use scrapy_rs_core::signal::{Signal, SignalArgs, SignalManager};
use scrapy_rs_engine::{Engine, EngineStats};
use scrapy_rs_engine::resource_control::ResourceStats;

// Re-export Result type
pub type Result<T> = std::result::Result<T, Error>;

// Define modules
pub mod metrics;
pub mod dashboard;
pub mod server;

// Re-export types
pub use metrics::{Metrics, MetricsCollector};
pub use dashboard::Dashboard;
pub use server::MonitoringServer;

/// Event types for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitoringEvent {
    /// Engine started
    EngineStarted,
    /// Engine stopped
    EngineStopped,
    /// Engine paused
    EnginePaused,
    /// Engine resumed
    EngineResumed,
    /// Spider opened
    SpiderOpened { name: String },
    /// Spider closed
    SpiderClosed { name: String },
    /// Request scheduled
    RequestScheduled { url: String },
    /// Request sent
    RequestSent { url: String },
    /// Response received
    ResponseReceived { url: String, status: u16 },
    /// Item scraped
    ItemScraped { item_type: String },
    /// Error occurred
    ErrorOccurred { message: String },
    /// Stats updated
    StatsUpdated { stats: EngineStats },
    /// Resource stats updated
    ResourceStatsUpdated { stats: metrics::ResourceStatsDto },
    /// Custom event
    Custom { name: String, data: serde_json::Value },
}

/// Monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Whether to enable monitoring
    pub enabled: bool,
    /// Interval for collecting metrics in milliseconds
    pub metrics_interval_ms: u64,
    /// Maximum number of events to keep in history
    pub max_events: usize,
    /// Whether to enable the web dashboard
    pub enable_dashboard: bool,
    /// Port for the web dashboard
    pub dashboard_port: u16,
    /// Whether to enable the metrics server
    pub enable_metrics_server: bool,
    /// Port for the metrics server
    pub metrics_server_port: u16,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            metrics_interval_ms: 1000,
            max_events: 1000,
            enable_dashboard: false,
            dashboard_port: 8080,
            enable_metrics_server: false,
            metrics_server_port: 9090,
        }
    }
}

/// Monitoring system for Scrapy-RS
pub struct Monitoring {
    /// Monitoring configuration
    config: MonitoringConfig,
    /// Event history
    events: Arc<RwLock<Vec<(DateTime<Utc>, MonitoringEvent)>>>,
    /// Event sender
    event_tx: broadcast::Sender<MonitoringEvent>,
    /// Metrics collector
    metrics_collector: Arc<MetricsCollector>,
    /// Dashboard server
    dashboard: Option<Dashboard>,
    /// Metrics server
    metrics_server: Option<server::MetricsServer>,
    /// Engine reference
    engine: Option<Arc<Engine>>,
    /// Signal manager reference
    signals: Option<Arc<SignalManager>>,
    /// Background tasks
    tasks: Vec<JoinHandle<()>>,
}

impl Monitoring {
    /// Create a new monitoring system with the given configuration
    pub fn new(config: MonitoringConfig) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        let events = Arc::new(RwLock::new(Vec::new()));
        let metrics_collector = Arc::new(MetricsCollector::new());
        
        let dashboard = if config.enable_dashboard {
            Some(Dashboard::new(
                config.dashboard_port,
                events.clone(),
                metrics_collector.clone(),
                event_tx.clone(),
            ))
        } else {
            None
        };
        
        let metrics_server = if config.enable_metrics_server {
            Some(server::MetricsServer::new(
                config.metrics_server_port,
                metrics_collector.clone(),
            ))
        } else {
            None
        };
        
        Self {
            config,
            events,
            event_tx,
            metrics_collector,
            dashboard,
            metrics_server,
            engine: None,
            signals: None,
            tasks: Vec::new(),
        }
    }
    
    /// Attach the monitoring system to an engine
    pub async fn attach(&mut self, engine: Arc<Engine>) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        self.engine = Some(engine.clone());
        self.signals = Some(engine.signals());
        
        // Register signal handlers
        let signals = self.signals.as_ref().unwrap().clone();
        let event_tx = self.event_tx.clone();
        
        signals.connect(Signal::EngineStarted, move |_| {
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let _ = event_tx.send(MonitoringEvent::EngineStarted);
            });
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::EngineStopped, move |_| {
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let _ = event_tx.send(MonitoringEvent::EngineStopped);
            });
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::EnginePaused, move |_| {
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let _ = event_tx.send(MonitoringEvent::EnginePaused);
            });
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::EngineResumed, move |_| {
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let _ = event_tx.send(MonitoringEvent::EngineResumed);
            });
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::SpiderOpened, move |args| {
            let event_tx = event_tx.clone();
            if let SignalArgs::Spider(spider) = args {
                let name = spider.name().to_string();
                tokio::spawn(async move {
                    let _ = event_tx.send(MonitoringEvent::SpiderOpened { name });
                });
            }
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::SpiderClosed, move |args| {
            let event_tx = event_tx.clone();
            if let SignalArgs::Spider(spider) = args {
                let name = spider.name().to_string();
                tokio::spawn(async move {
                    let _ = event_tx.send(MonitoringEvent::SpiderClosed { name });
                });
            }
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::RequestScheduled, move |args| {
            let event_tx = event_tx.clone();
            if let SignalArgs::Request(request) = args {
                let url = request.url.to_string();
                tokio::spawn(async move {
                    let _ = event_tx.send(MonitoringEvent::RequestScheduled { url });
                });
            }
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::RequestSent, move |args| {
            let event_tx = event_tx.clone();
            if let SignalArgs::Request(request) = args {
                let url = request.url.to_string();
                tokio::spawn(async move {
                    let _ = event_tx.send(MonitoringEvent::RequestSent { url });
                });
            }
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::ResponseReceived, move |args| {
            let event_tx = event_tx.clone();
            if let SignalArgs::Response(response) = args {
                let url = response.url.to_string();
                let status = response.status;
                tokio::spawn(async move {
                    let _ = event_tx.send(MonitoringEvent::ResponseReceived { url, status });
                });
            }
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::ItemScraped, move |args| {
            let event_tx = event_tx.clone();
            if let SignalArgs::Item(item) = args {
                let item_type = item.item_type_name.clone();
                tokio::spawn(async move {
                    let _ = event_tx.send(MonitoringEvent::ItemScraped { item_type });
                });
            }
            Ok(())
        }).await?;
        
        let event_tx = self.event_tx.clone();
        signals.connect(Signal::ErrorOccurred, move |args| {
            let event_tx = event_tx.clone();
            if let SignalArgs::Error(message) = args {
                let message = message.clone();
                tokio::spawn(async move {
                    let _ = event_tx.send(MonitoringEvent::ErrorOccurred { message });
                });
            }
            Ok(())
        }).await?;
        
        // Start event listener
        let events = self.events.clone();
        let max_events = self.config.max_events;
        let mut event_rx = self.event_tx.subscribe();
        
        let event_task = tokio::spawn(async move {
            while let Ok(event) = event_rx.recv().await {
                let now = Utc::now();
                let mut events_write = events.write().await;
                events_write.push((now, event));
                
                // Trim events if needed
                if events_write.len() > max_events {
                    events_write.drain(0..events_write.len() - max_events);
                }
            }
        });
        
        self.tasks.push(event_task);
        
        // Start metrics collector
        let engine_clone = engine.clone();
        let metrics_collector = self.metrics_collector.clone();
        let interval_ms = self.config.metrics_interval_ms;
        let event_tx = self.event_tx.clone();
        
        let metrics_task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(interval_ms));
            
            loop {
                interval.tick().await;
                
                // Collect engine stats
                let stats = engine_clone.stats().await;
                metrics_collector.update_engine_stats(&stats).await;
                
                // Send stats updated event
                let _ = event_tx.send(MonitoringEvent::StatsUpdated { stats: stats.clone() });
                
                // Collect resource stats if available
                if let Some(resource_stats) = engine_clone.get_resource_stats().await {
                    metrics_collector.update_resource_stats(&resource_stats).await;
                    
                    // Send resource stats updated event
                    let stats_dto = metrics::ResourceStatsDto::from(&resource_stats);
                    let _ = event_tx.send(MonitoringEvent::ResourceStatsUpdated { stats: stats_dto });
                }
            }
        });
        
        self.tasks.push(metrics_task);
        
        // Start dashboard if enabled
        if let Some(dashboard) = &self.dashboard {
            let dashboard_task = dashboard.start().await?;
            self.tasks.push(dashboard_task);
        }
        
        // Start metrics server if enabled
        if let Some(metrics_server) = &self.metrics_server {
            let metrics_server_task = metrics_server.start().await?;
            self.tasks.push(metrics_server_task);
        }
        
        Ok(())
    }
    
    /// Send a custom event
    pub fn send_event(&self, name: &str, data: serde_json::Value) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        let event = MonitoringEvent::Custom {
            name: name.to_string(),
            data,
        };
        
        self.event_tx.send(event).map_err(|e| {
            Error::Other {
                message: format!("Failed to send monitoring event: {}", e),
                context: Default::default(),
            }
        })?;
        
        Ok(())
    }
    
    /// Get the event history
    pub async fn get_events(&self) -> Vec<(DateTime<Utc>, MonitoringEvent)> {
        let events = self.events.read().await;
        events.clone()
    }
    
    /// Get the current metrics
    pub async fn get_metrics(&self) -> Metrics {
        self.metrics_collector.get_metrics().await
    }
    
    /// Stop the monitoring system
    pub async fn stop(&mut self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        // Abort all tasks
        for task in self.tasks.drain(..) {
            task.abort();
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
