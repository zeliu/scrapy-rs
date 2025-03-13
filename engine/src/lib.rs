use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use log::{debug, error, info, warn};
use scrapy_rs_core::error::{Error, ErrorContext, Result};
use scrapy_rs_core::error_handler::{ErrorAction, ErrorManager};
use scrapy_rs_core::item::DynamicItem;
use scrapy_rs_core::request::Request;
use scrapy_rs_core::signal::{Signal, SignalArgs, SignalManager};
use scrapy_rs_core::spider::Spider;
use scrapy_rs_downloader::{Downloader, DownloaderConfig, HttpDownloader};
use scrapy_rs_middleware::{
    ChainedRequestMiddleware, ChainedResponseMiddleware, DefaultHeadersMiddleware,
    RequestMiddleware, ResponseLoggerMiddleware, ResponseMiddleware, RetryMiddleware,
};
use scrapy_rs_pipeline::{DummyPipeline, LogPipeline, Pipeline, PipelineType};
use scrapy_rs_scheduler::{
    BreadthFirstScheduler, CrawlStrategy, DepthFirstScheduler, DomainGroupScheduler,
    MemoryScheduler, Scheduler, SchedulerConfig,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Notify, RwLock};
use tokio::time::sleep;

pub mod resource_control;
pub use resource_control::{ResourceController, ResourceLimits, ResourceStats};

/// Scheduler type to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerType {
    /// Memory-based scheduler with priority queue
    Memory,
    /// Domain-aware scheduler with per-domain queues
    DomainGroup,
    /// Breadth-first search scheduler
    BreadthFirst,
    /// Depth-first search scheduler
    DepthFirst,
}

impl Default for SchedulerType {
    fn default() -> Self {
        Self::Memory
    }
}

/// Configuration for the crawler engine
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Maximum number of concurrent requests
    pub concurrent_requests: usize,

    /// Maximum number of items to process concurrently
    pub concurrent_items: usize,

    /// Delay between requests in milliseconds
    pub download_delay_ms: u64,

    /// Maximum number of requests per domain
    pub max_requests_per_domain: Option<usize>,

    /// Maximum number of requests per spider
    pub max_requests_per_spider: Option<usize>,

    /// Whether to respect robots.txt
    pub respect_robots_txt: bool,

    /// User agent string
    pub user_agent: String,

    /// Default request timeout in seconds
    pub request_timeout: u64,

    /// Whether to retry failed requests
    pub retry_enabled: bool,

    /// Maximum number of retries
    pub max_retries: u32,

    /// Whether to follow redirects
    pub follow_redirects: bool,

    /// Maximum depth to crawl
    pub max_depth: Option<usize>,

    /// Whether to log requests and responses
    pub log_requests: bool,

    /// Whether to log items
    pub log_items: bool,

    /// Whether to log stats
    pub log_stats: bool,

    /// Interval for logging stats in seconds
    pub stats_interval_secs: u64,

    /// Type of scheduler to use
    pub scheduler_type: SchedulerType,

    /// Crawl strategy to use
    pub crawl_strategy: CrawlStrategy,

    /// Delay between requests to the same domain (in milliseconds)
    pub domain_delay_ms: Option<u64>,

    /// Resource limits
    pub resource_limits: ResourceLimits,

    /// Enable resource monitoring
    pub enable_resource_monitoring: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            concurrent_requests: 16,
            concurrent_items: 100,
            download_delay_ms: 0,
            max_requests_per_domain: None,
            max_requests_per_spider: None,
            respect_robots_txt: true,
            user_agent: format!("scrapy_rs/{}", env!("CARGO_PKG_VERSION")),
            request_timeout: 30,
            retry_enabled: true,
            max_retries: 3,
            follow_redirects: true,
            max_depth: None,
            log_requests: true,
            log_items: true,
            log_stats: true,
            stats_interval_secs: 60,
            scheduler_type: SchedulerType::Memory,
            crawl_strategy: CrawlStrategy::Priority,
            domain_delay_ms: None,
            resource_limits: ResourceLimits::default(),
            enable_resource_monitoring: false,
        }
    }
}

/// Statistics for the crawler engine
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineStats {
    /// Number of requests sent
    pub request_count: usize,

    /// Number of responses received
    pub response_count: usize,

    /// Number of items scraped
    pub item_count: usize,

    /// Number of errors
    pub error_count: usize,

    /// Start time of the crawl
    #[serde(skip)]
    pub start_time: Option<Instant>,

    /// End time of the crawl
    #[serde(skip)]
    pub end_time: Option<Instant>,

    /// Duration in seconds (for serialization)
    #[serde(skip_deserializing)]
    pub duration_secs: Option<f64>,
}

impl EngineStats {
    /// Get the duration of the crawl
    pub fn duration(&self) -> Option<Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            (Some(start), None) => Some(Instant::now().duration_since(start)),
            _ => None,
        }
    }

    /// Get the requests per second
    pub fn requests_per_second(&self) -> Option<f64> {
        self.duration().map(|duration| {
            let secs = duration.as_secs_f64();
            if secs > 0.0 {
                self.request_count as f64 / secs
            } else {
                0.0
            }
        })
    }

    /// Prepare for serialization
    pub fn prepare_for_serialization(&mut self) {
        if let Some(duration) = self.duration() {
            self.duration_secs = Some(duration.as_secs_f64());
        }
    }

    /// Restore after deserialization
    pub fn after_deserialization(&mut self) {
        // Set start time to current time, so we can continue timing
        self.start_time = Some(Instant::now());
        self.end_time = None;
    }
}

/// Engine state, used for saving and restoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineState {
    /// Engine statistics
    pub stats: EngineStats,
    // More state can be added here
}

/// The crawler engine
pub struct Engine {
    /// The spider to use
    spider: Arc<dyn Spider>,

    /// The scheduler to use
    scheduler: Arc<dyn Scheduler>,

    /// The downloader to use
    downloader: Arc<dyn Downloader>,

    /// The item pipelines to use
    pipelines: Arc<dyn Pipeline>,

    /// The request middlewares to use
    request_middlewares: Arc<dyn RequestMiddleware>,

    /// The response middlewares to use
    response_middlewares: Arc<dyn ResponseMiddleware>,

    /// The signal manager
    signals: Arc<SignalManager>,

    /// The engine configuration
    config: EngineConfig,

    /// The engine statistics
    stats: Arc<RwLock<EngineStats>>,

    /// Whether the engine is running
    running: Arc<RwLock<bool>>,

    /// Channel for sending items to the pipeline
    item_tx: Option<mpsc::Sender<DynamicItem>>,

    /// Channel for receiving items from the pipeline
    item_rx: Option<mpsc::Receiver<DynamicItem>>,

    /// Add a Notify for pause/resume notification
    pause_notify: Arc<Notify>,

    /// Error manager for handling errors
    error_manager: Arc<ErrorManager>,

    /// Resource controller
    resource_controller: Option<ResourceController>,
}

impl Engine {
    /// Create a new engine with the given spider and default components
    pub fn new(spider: Arc<dyn Spider>) -> Result<Self> {
        let config = EngineConfig::default();

        // Create the scheduler based on the configuration
        let scheduler = Self::create_scheduler(&config)?;

        // Create the downloader
        let downloader_config = DownloaderConfig {
            concurrent_requests: config.concurrent_requests,
            user_agent: config.user_agent.clone(),
            timeout: config.request_timeout,
            retry_enabled: config.retry_enabled,
            max_retries: config.max_retries,
            ..DownloaderConfig::default()
        };
        let downloader = Arc::new(HttpDownloader::new(downloader_config)?);

        // Create the pipelines
        let mut pipelines = Vec::new();

        if config.log_items {
            pipelines.push(PipelineType::Log(LogPipeline::info()));
        }

        let pipeline = if pipelines.is_empty() {
            Arc::new(PipelineType::Dummy(DummyPipeline::new())) as Arc<dyn Pipeline>
        } else {
            Arc::new(PipelineType::Chained(pipelines)) as Arc<dyn Pipeline>
        };

        // Create the request middlewares
        let mut request_middlewares = ChainedRequestMiddleware::new(vec![]);

        // Add default headers middleware
        let mut headers = HashMap::new();
        headers.insert("User-Agent".to_string(), config.user_agent.clone());
        request_middlewares.add(DefaultHeadersMiddleware::new(headers));

        // Create the response middlewares
        let mut response_middlewares = ChainedResponseMiddleware::new(vec![]);

        if config.log_requests {
            response_middlewares.add(ResponseLoggerMiddleware::info());
        }

        if config.retry_enabled {
            response_middlewares.add(RetryMiddleware::common());
        }

        // Create the signal manager
        let signals = Arc::new(SignalManager::new());

        // Create the error manager
        let error_manager = Arc::new(ErrorManager::default());

        // Create the item channels
        let (item_tx, item_rx) = mpsc::channel(config.concurrent_items);

        // Create resource controller if enabled
        let resource_controller = if config.enable_resource_monitoring {
            Some(ResourceController::new(config.resource_limits.clone()))
        } else {
            None
        };

        Ok(Self {
            spider,
            scheduler,
            downloader,
            pipelines: pipeline,
            request_middlewares: Arc::new(request_middlewares),
            response_middlewares: Arc::new(response_middlewares),
            signals,
            config,
            stats: Arc::new(RwLock::new(EngineStats::default())),
            running: Arc::new(RwLock::new(false)),
            item_tx: Some(item_tx),
            item_rx: Some(item_rx),
            pause_notify: Arc::new(Notify::new()),
            error_manager,
            resource_controller,
        })
    }

    /// Create a scheduler based on the engine configuration
    fn create_scheduler(config: &EngineConfig) -> Result<Arc<dyn Scheduler>> {
        match config.scheduler_type {
            SchedulerType::Memory => Ok(Arc::new(MemoryScheduler::new())),
            SchedulerType::DomainGroup => {
                let scheduler_config = SchedulerConfig {
                    strategy: config.crawl_strategy,
                    max_requests_per_domain: config.max_requests_per_domain,
                    domain_delay_ms: config.domain_delay_ms,
                    domain_whitelist: None,
                    domain_blacklist: None,
                    respect_depth: true,
                    max_depth: config.max_depth,
                };
                Ok(Arc::new(DomainGroupScheduler::new(scheduler_config)))
            }
            SchedulerType::BreadthFirst => {
                Ok(Arc::new(BreadthFirstScheduler::new(config.max_depth)))
            }
            SchedulerType::DepthFirst => Ok(Arc::new(DepthFirstScheduler::new(config.max_depth))),
        }
    }

    /// Create a new engine with custom components
    pub fn with_components(
        spider: Arc<dyn Spider>,
        scheduler: Arc<dyn Scheduler>,
        downloader: Arc<dyn Downloader>,
        pipelines: Arc<dyn Pipeline>,
        request_middlewares: Arc<dyn RequestMiddleware>,
        response_middlewares: Arc<dyn ResponseMiddleware>,
        config: EngineConfig,
    ) -> Self {
        // Create the item channels
        let (item_tx, item_rx) = mpsc::channel(config.concurrent_items);

        // Create the signal manager
        let signals = Arc::new(SignalManager::new());

        // Create the error manager
        let error_manager = Arc::new(ErrorManager::default());

        // Create resource controller if enabled
        let resource_controller = if config.enable_resource_monitoring {
            Some(ResourceController::new(config.resource_limits.clone()))
        } else {
            None
        };

        Self {
            spider,
            scheduler,
            downloader,
            pipelines,
            request_middlewares,
            response_middlewares,
            signals,
            config,
            stats: Arc::new(RwLock::new(EngineStats::default())),
            running: Arc::new(RwLock::new(false)),
            item_tx: Some(item_tx),
            item_rx: Some(item_rx),
            pause_notify: Arc::new(Notify::new()),
            error_manager,
            resource_controller,
        }
    }

    /// Set the engine configuration
    pub fn with_config(mut self, config: EngineConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the signal manager
    pub fn signals(&self) -> Arc<SignalManager> {
        self.signals.clone()
    }

    /// Run the engine until all requests are processed
    pub async fn run(&mut self) -> Result<EngineStats> {
        // Check if the engine is already running
        let mut running = self.running.write().await;
        if *running {
            return Err(Box::new(Error::Other {
                message: "Engine is already running".to_string(),
                context: ErrorContext::new(),
            }));
        }
        *running = true;
        drop(running);

        // Initialize stats
        let mut stats = self.stats.write().await;
        stats.start_time = Some(Instant::now());
        drop(stats);

        // Start resource monitoring if enabled
        if let Some(ref controller) = self.resource_controller {
            controller.start().await;
        }

        // Register signal handlers
        let running_clone = self.running.clone();
        self.signals
            .connect(Signal::EnginePaused, move |_| {
                let running = running_clone.clone();
                tokio::spawn(async move {
                    let mut r = running.write().await;
                    *r = false;
                    info!("Engine paused by signal");
                });
                Ok(())
            })
            .await?;

        let running_clone = self.running.clone();
        self.signals
            .connect(Signal::EngineResumed, move |_| {
                let running = running_clone.clone();
                tokio::spawn(async move {
                    let mut r = running.write().await;
                    *r = true;
                    info!("Engine resumed by signal");
                });
                Ok(())
            })
            .await?;

        // Send engine started signal
        self.signals
            .send_catch_log(Signal::EngineStarted, SignalArgs::None)
            .await;

        // Open the spider
        info!("Opening spider: {}", self.spider.name());
        self.request_middlewares
            .spider_opened(&*self.spider)
            .await?;
        self.response_middlewares
            .spider_opened(&*self.spider)
            .await?;
        self.pipelines.open_spider(&*self.spider).await?;

        // Send spider opened signal
        self.signals
            .send_catch_log(
                Signal::SpiderOpened,
                SignalArgs::Spider(self.spider.clone()),
            )
            .await;

        // Add start requests to the scheduler
        info!("Adding start requests for spider: {}", self.spider.name());
        let start_requests = self.spider.start_requests();
        for request_result in start_requests {
            match request_result {
                Ok(request) => {
                    // Send request scheduled signal
                    self.signals
                        .send_catch_log(
                            Signal::RequestScheduled,
                            SignalArgs::Request(Box::new(request.clone())),
                        )
                        .await;

                    self.scheduler.enqueue(request).await?;
                }
                Err(e) => {
                    error!("Error creating start request: {}", e);
                    let mut stats = self.stats.write().await;
                    stats.error_count += 1;

                    // Handle error from start_requests
                    let error_with_context =
                        Error::other(format!("Error creating start request: {}", e))
                            .with_component("spider_start_requests")
                            .with_spider_name(self.spider.name());

                    match self
                        .error_manager
                        .handle_error(&error_with_context, &*self.spider)
                        .await
                    {
                        Ok(ErrorAction::Retry {
                            request,
                            delay,
                            reason,
                        }) => {
                            info!("Retrying start request: {}", reason);

                            // Apply delay if specified
                            if let Some(delay_duration) = delay {
                                sleep(delay_duration).await;
                            }

                            // Send request scheduled signal
                            self.signals
                                .send_catch_log(
                                    Signal::RequestScheduled,
                                    SignalArgs::Request(request.clone()),
                                )
                                .await;

                            if let Err(e) = self.scheduler.enqueue(*request).await {
                                error!("Error enqueueing retry start request: {}", e);
                            }
                        }
                        Ok(ErrorAction::Abort { reason }) => {
                            error!("Aborting crawl: {}", reason);
                            return Err(Box::new(Error::Other {
                                message: format!("Aborted: {}", reason),
                                context: ErrorContext::new(),
                            }));
                        }
                        Ok(ErrorAction::Skip { reason }) => {
                            warn!("Skipping start request: {}", reason);
                        }
                        Ok(ErrorAction::Ignore) => {
                            debug!("Ignoring start request error: {}", error_with_context);
                        }
                        Err(e) => {
                            error!("Error handling start request error: {}", e);
                        }
                    }

                    // Send error signal
                    self.signals
                        .send_catch_log(
                            Signal::ErrorOccurred,
                            SignalArgs::Error(format!("Error creating start request: {}", e)),
                        )
                        .await;
                }
            }
        }

        // Start the stats logger task
        let stats_task = if self.config.log_stats {
            let stats = self.stats.clone();
            let interval = self.config.stats_interval_secs;
            let running = self.running.clone();

            Some(tokio::spawn(async move {
                while *running.read().await {
                    sleep(Duration::from_secs(interval)).await;
                    let stats = stats.read().await;

                    if let Some(duration) = stats.duration() {
                        let rps = stats.requests_per_second().unwrap_or(0.0);
                        info!(
                            "Stats: {} requests, {} responses, {} items, {} errors, {:.2} req/s, {:.2}s elapsed",
                            stats.request_count,
                            stats.response_count,
                            stats.item_count,
                            stats.error_count,
                            rps,
                            duration.as_secs_f64(),
                        );
                    }
                }
            }))
        } else {
            None
        };

        // Start the item processor task
        let item_processor_task = {
            let pipelines = self.pipelines.clone();
            let spider = self.spider.clone();
            let stats = self.stats.clone();
            let mut item_rx = self.item_rx.take().unwrap();
            let running = self.running.clone();
            let pause_notify = self.pause_notify.clone();
            let error_manager = self.error_manager.clone();

            tokio::spawn(async move {
                while let Some(item) = item_rx.recv().await {
                    // Check if paused
                    if !*running.read().await {
                        // Wait for resume notification, instead of polling
                        pause_notify.notified().await;
                    }

                    match pipelines.process_dynamic_item(item.clone(), &*spider).await {
                        Ok(_) => {
                            let mut stats = stats.write().await;
                            stats.item_count += 1;
                        }
                        Err(e) => {
                            error!("Error processing item: {}", e);
                            let mut stats = stats.write().await;
                            stats.error_count += 1;

                            // Handle error from request middleware
                            let error_with_context =
                                Error::other(format!("Error processing item: {}", e))
                                    .with_component("pipeline")
                                    .with_spider_name(spider.name());

                            match error_manager
                                .handle_error(&error_with_context, &*spider)
                                .await
                            {
                                Ok(ErrorAction::Retry {
                                    request: _,
                                    delay: _,
                                    reason,
                                }) => {
                                    // We can't retry item processing directly, but we can log it
                                    warn!("Item processing error marked for retry, but retry not implemented: {}", reason);
                                }
                                Ok(ErrorAction::Abort { reason }) => {
                                    error!("Item processing error marked for abort: {}", reason);
                                    // We can't abort from here, but we can log it
                                }
                                Ok(ErrorAction::Skip { reason }) => {
                                    warn!("Skipping item: {}", reason);
                                }
                                Ok(ErrorAction::Ignore) => {
                                    debug!(
                                        "Ignoring item processing error: {}",
                                        error_with_context
                                    );
                                }
                                Err(e) => {
                                    error!("Error handling item processing error: {}", e);
                                }
                            }
                        }
                    }
                }
            })
        };

        // Process requests until the scheduler is empty
        let mut active_tasks = 0;
        let mut tasks = FuturesUnordered::new();

        'main_loop: while !self.scheduler.is_empty().await || active_tasks > 0 || !tasks.is_empty()
        {
            // Check if paused
            if !*self.running.read().await {
                info!("Engine is paused, waiting for resume...");
                // Wait for resume notification, instead of polling
                self.pause_notify.notified().await;
                info!("Engine resumed, continuing...");
            }

            // Check if we can start more tasks
            while active_tasks < self.config.concurrent_requests && !self.scheduler.is_empty().await
            {
                // Check if paused
                if !*self.running.read().await {
                    break;
                }

                if let Some(request) = self.scheduler.next().await {
                    // Process the request through middleware
                    let processed_request = match self
                        .request_middlewares
                        .process_request(request, &*self.spider)
                        .await
                    {
                        Ok(req) => req,
                        Err(e) => {
                            // Handle error from request middleware
                            let error_with_context =
                                Error::other(format!("Request middleware rejected request: {}", e))
                                    .with_component("request_middleware")
                                    .with_spider_name(self.spider.name());

                            // Log the error
                            error!("Request middleware rejected request: {}", e);

                            warn!("Request middleware rejected request: {}", e);

                            let mut stats = self.stats.write().await;
                            stats.error_count += 1;

                            match self
                                .error_manager
                                .handle_error(&error_with_context, &*self.spider)
                                .await
                            {
                                Ok(ErrorAction::Retry {
                                    request,
                                    delay,
                                    reason,
                                }) => {
                                    info!("Retrying request: {}", reason);

                                    // Apply delay if specified
                                    if let Some(delay_duration) = delay {
                                        sleep(delay_duration).await;
                                    }

                                    // Send request scheduled signal
                                    self.signals
                                        .send_catch_log(
                                            Signal::RequestScheduled,
                                            SignalArgs::Request(request.clone()),
                                        )
                                        .await;

                                    if let Err(e) = self.scheduler.enqueue(*request).await {
                                        error!("Error enqueueing retry request: {}", e);
                                    }
                                }
                                Ok(ErrorAction::Abort { reason }) => {
                                    error!("Aborting crawl: {}", reason);
                                    break 'main_loop;
                                }
                                Ok(ErrorAction::Skip { reason }) => {
                                    warn!("Skipping request: {}", reason);
                                }
                                Ok(ErrorAction::Ignore) => {
                                    debug!("Ignoring error: {}", error_with_context);
                                }
                                Err(e) => {
                                    error!("Error handling error: {}", e);
                                }
                            }

                            // Send error signal
                            self.signals
                                .send_catch_log(
                                    Signal::ErrorOccurred,
                                    SignalArgs::Error(format!(
                                        "Request middleware rejected request: {}",
                                        e
                                    )),
                                )
                                .await;

                            continue;
                        }
                    };

                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.request_count += 1;
                    }

                    // Send request sent signal
                    let req_clone = processed_request.clone();
                    self.signals
                        .send_catch_log(
                            Signal::RequestSent,
                            SignalArgs::Request(Box::new(req_clone)),
                        )
                        .await;

                    // Download the request
                    let downloader = self.downloader.clone();
                    let response_middlewares = self.response_middlewares.clone();
                    let spider = self.spider.clone();
                    let stats = self.stats.clone();
                    let scheduler = self.scheduler.clone();
                    let item_tx = self.item_tx.clone().unwrap();
                    let signals = self.signals.clone();
                    let running = self.running.clone();
                    let error_manager = self.error_manager.clone();

                    // Add a Notify for pause/resume notification
                    let task_pause_notify = self.pause_notify.clone();

                    let task = tokio::spawn(async move {
                        // Check if paused
                        if !*running.read().await {
                            // Wait for resume notification, instead of polling
                            task_pause_notify.notified().await;
                        }

                        // Download the request
                        let response = match downloader.download(processed_request.clone()).await {
                            Ok(resp) => resp,
                            Err(e) => {
                                // Handle error from downloader
                                let error_with_context =
                                    Error::other(format!("Error downloading request: {}", e))
                                        .with_component("downloader")
                                        .with_spider_name(spider.name())
                                        .with_url(processed_request.url.to_string());

                                // Log the error
                                error!("Error downloading request: {}", e);

                                let mut stats = stats.write().await;
                                stats.error_count += 1;

                                match error_manager
                                    .handle_error(&error_with_context, &*spider)
                                    .await
                                {
                                    Ok(ErrorAction::Retry {
                                        request,
                                        delay,
                                        reason,
                                    }) => {
                                        info!("Retrying request: {}", reason);

                                        // Apply delay if specified
                                        if let Some(delay_duration) = delay {
                                            sleep(delay_duration).await;
                                        }

                                        // Send request scheduled signal
                                        signals
                                            .send_catch_log(
                                                Signal::RequestScheduled,
                                                SignalArgs::Request(request.clone()),
                                            )
                                            .await;

                                        if let Err(e) = scheduler.enqueue(*request).await {
                                            error!("Error enqueueing retry request: {}", e);
                                        }
                                    }
                                    Ok(ErrorAction::Abort { reason }) => {
                                        error!("Aborting crawl: {}", reason);
                                        // We can't abort from here, but we can log it
                                    }
                                    Ok(ErrorAction::Skip { reason }) => {
                                        warn!("Skipping request: {}", reason);
                                    }
                                    Ok(ErrorAction::Ignore) => {
                                        debug!("Ignoring error: {}", error_with_context);
                                    }
                                    Err(e) => {
                                        error!("Error handling error: {}", e);
                                    }
                                }

                                // Send error signal
                                signals
                                    .send_catch_log(
                                        Signal::ErrorOccurred,
                                        SignalArgs::Error(format!(
                                            "Error downloading request: {}",
                                            e
                                        )),
                                    )
                                    .await;

                                return;
                            }
                        };

                        // Check if paused
                        if !*running.read().await {
                            // Wait for resume notification, instead of polling
                            task_pause_notify.notified().await;
                        }

                        // Update stats
                        {
                            let mut stats = stats.write().await;
                            stats.response_count += 1;
                        }

                        // Send response received signal
                        let resp_clone = response.clone();
                        signals
                            .send_catch_log(
                                Signal::ResponseReceived,
                                SignalArgs::Response(Box::new(resp_clone)),
                            )
                            .await;

                        // Process the response through middleware
                        let processed_response = match response_middlewares
                            .process_response(response, &*spider)
                            .await
                        {
                            Ok(resp) => resp,
                            Err(e) => {
                                // Handle error from response middleware
                                let error_with_context = Error::other(format!(
                                    "Response middleware rejected response: {}",
                                    e
                                ))
                                .with_component("response_middleware")
                                .with_spider_name(spider.name());

                                // Log the error
                                error!("Response middleware rejected response: {}", e);

                                warn!("Response middleware rejected response: {}", e);

                                let mut stats = stats.write().await;
                                stats.error_count += 1;

                                match error_manager
                                    .handle_error(&error_with_context, &*spider)
                                    .await
                                {
                                    Ok(ErrorAction::Retry {
                                        request,
                                        delay,
                                        reason,
                                    }) => {
                                        info!("Retrying request: {}", reason);

                                        // Apply delay if specified
                                        if let Some(delay_duration) = delay {
                                            sleep(delay_duration).await;
                                        }

                                        // Send request scheduled signal
                                        signals
                                            .send_catch_log(
                                                Signal::RequestScheduled,
                                                SignalArgs::Request(request.clone()),
                                            )
                                            .await;

                                        if let Err(e) = scheduler.enqueue(*request).await {
                                            error!("Error enqueueing retry request: {}", e);
                                        }
                                    }
                                    Ok(ErrorAction::Abort { reason }) => {
                                        error!("Aborting crawl: {}", reason);
                                        // We can't abort from here, but we can log it
                                    }
                                    Ok(ErrorAction::Skip { reason }) => {
                                        warn!("Skipping request: {}", reason);
                                    }
                                    Ok(ErrorAction::Ignore) => {
                                        debug!("Ignoring error: {}", error_with_context);
                                    }
                                    Err(e) => {
                                        error!("Error handling error: {}", e);
                                    }
                                }

                                // Send error signal
                                signals
                                    .send_catch_log(
                                        Signal::ErrorOccurred,
                                        SignalArgs::Error(format!(
                                            "Response middleware rejected response: {}",
                                            e
                                        )),
                                    )
                                    .await;

                                return;
                            }
                        };

                        // Check if paused
                        if !*running.read().await {
                            // Wait for resume notification, instead of polling
                            task_pause_notify.notified().await;
                        }

                        // Check if we need to retry the request
                        if let Some(retry_request) = processed_response.meta.get("retry_request") {
                            if let Ok(request) =
                                serde_json::from_value::<Request>(retry_request.clone())
                            {
                                // Send request scheduled signal for retry
                                signals
                                    .send_catch_log(
                                        Signal::RequestScheduled,
                                        SignalArgs::Request(Box::new(request.clone())),
                                    )
                                    .await;

                                if let Err(e) = scheduler.enqueue(request).await {
                                    error!("Error enqueueing retry request: {}", e);
                                    let mut stats = stats.write().await;
                                    stats.error_count += 1;

                                    // Send error signal
                                    signals
                                        .send_catch_log(
                                            Signal::ErrorOccurred,
                                            SignalArgs::Error(format!(
                                                "Error enqueueing retry request: {}",
                                                e
                                            )),
                                        )
                                        .await;
                                }
                                return;
                            }
                        }

                        // Check if paused
                        if !*running.read().await {
                            // Wait for resume notification, instead of polling
                            task_pause_notify.notified().await;
                        }

                        // Parse the response
                        let parse_output = match spider.parse(processed_response.clone()).await {
                            Ok(output) => output,
                            Err(e) => {
                                // Handle error from spider parse
                                let error_with_context =
                                    Error::other(format!("Error parsing response: {}", e))
                                        .with_component("spider_parse")
                                        .with_spider_name(spider.name())
                                        .with_url(processed_response.url.to_string());

                                // Log the error
                                error!("Error parsing response: {}", e);

                                let mut stats = stats.write().await;
                                stats.error_count += 1;

                                match error_manager
                                    .handle_error(&error_with_context, &*spider)
                                    .await
                                {
                                    Ok(ErrorAction::Retry {
                                        request,
                                        delay,
                                        reason,
                                    }) => {
                                        info!("Retrying request: {}", reason);

                                        // Apply delay if specified
                                        if let Some(delay_duration) = delay {
                                            sleep(delay_duration).await;
                                        }

                                        // Send request scheduled signal
                                        signals
                                            .send_catch_log(
                                                Signal::RequestScheduled,
                                                SignalArgs::Request(request.clone()),
                                            )
                                            .await;

                                        if let Err(e) = scheduler.enqueue(*request).await {
                                            error!("Error enqueueing retry request: {}", e);
                                        }
                                    }
                                    Ok(ErrorAction::Abort { reason }) => {
                                        error!("Aborting crawl: {}", reason);
                                        // We can't abort from here, but we can log it
                                    }
                                    Ok(ErrorAction::Skip { reason }) => {
                                        warn!("Skipping request: {}", reason);
                                    }
                                    Ok(ErrorAction::Ignore) => {
                                        debug!("Ignoring error: {}", error_with_context);
                                    }
                                    Err(e) => {
                                        error!("Error handling error: {}", e);
                                    }
                                }

                                // Send error signal
                                signals
                                    .send_catch_log(
                                        Signal::ErrorOccurred,
                                        SignalArgs::Error(format!("Error parsing response: {}", e)),
                                    )
                                    .await;

                                return;
                            }
                        };

                        // Check if paused
                        if !*running.read().await {
                            // Wait for resume notification, instead of polling
                            task_pause_notify.notified().await;
                        }

                        // Process the parse output
                        // Add new requests to the scheduler
                        for request in parse_output.requests {
                            // Send request scheduled signal
                            signals
                                .send_catch_log(
                                    Signal::RequestScheduled,
                                    SignalArgs::Request(Box::new(request.clone())),
                                )
                                .await;

                            if let Err(e) = scheduler.enqueue(request).await {
                                error!("Error enqueueing request: {}", e);
                                let mut stats = stats.write().await;
                                stats.error_count += 1;

                                // Send error signal
                                signals
                                    .send_catch_log(
                                        Signal::ErrorOccurred,
                                        SignalArgs::Error(format!(
                                            "Error enqueueing request: {}",
                                            e
                                        )),
                                    )
                                    .await;
                            }
                        }

                        // Check if paused
                        if !*running.read().await {
                            // Wait for resume notification, instead of polling
                            task_pause_notify.notified().await;
                        }

                        // Send items to the pipeline
                        for item in parse_output.items {
                            // Send item scraped signal
                            signals
                                .send_catch_log(Signal::ItemScraped, SignalArgs::Item(item.clone()))
                                .await;

                            if let Err(e) = item_tx.send(item).await {
                                error!("Error sending item to pipeline: {}", e);
                                let mut stats = stats.write().await;
                                stats.error_count += 1;

                                // Send error signal
                                signals
                                    .send_catch_log(
                                        Signal::ErrorOccurred,
                                        SignalArgs::Error(format!(
                                            "Error sending item to pipeline: {}",
                                            e
                                        )),
                                    )
                                    .await;
                            }
                        }
                    });

                    tasks.push(task);
                    active_tasks += 1;

                    // Add delay between requests if configured
                    if self.config.download_delay_ms > 0 {
                        sleep(Duration::from_millis(self.config.download_delay_ms)).await;
                    }
                }
            }

            // Wait for a task to complete
            if !tasks.is_empty() {
                if tasks.next().await.is_some() {
                    active_tasks -= 1;
                }
            } else if active_tasks == 0 && self.scheduler.is_empty().await {
                // No active tasks, but scheduler is empty, so we're done
                break;
            } else {
                // Wait for a short period to avoid CPU spinning
                sleep(Duration::from_millis(10)).await;
            }

            // Update resource stats
            if let Some(ref controller) = self.resource_controller {
                let pending_count = self.scheduler.len().await;
                controller.update_pending_requests(pending_count).await;

                let active_count = active_tasks;
                controller.update_active_tasks(active_count).await;

                // Apply throttling if needed
                controller.throttle_if_needed().await;
            }
        }

        // Wait for all tasks to complete
        while (tasks.next().await).is_some() {
            active_tasks -= 1;
        }

        // Close the item channel
        drop(self.item_tx.take());

        // Wait for the item processor to complete
        item_processor_task.await.map_err(|e| {
            Box::new(Error::Other {
                message: format!("Item processor task failed: {}", e),
                context: ErrorContext::new(),
            })
        })?;

        // Close the spider
        info!("Closing spider: {}", self.spider.name());
        self.pipelines.close_spider(&*self.spider).await?;
        self.response_middlewares
            .spider_closed(&*self.spider)
            .await?;
        self.request_middlewares
            .spider_closed(&*self.spider)
            .await?;
        self.spider.closed().await?;

        // Send spider closed signal
        self.signals
            .send_catch_log(
                Signal::SpiderClosed,
                SignalArgs::Spider(self.spider.clone()),
            )
            .await;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.end_time = Some(Instant::now());
        let stats_clone = stats.clone();
        drop(stats);

        // Stop the stats logger
        if let Some(task) = stats_task {
            let mut running = self.running.write().await;
            *running = false;
            drop(running);

            task.await.map_err(|e| {
                Box::new(Error::Other {
                    message: format!("Stats logger task failed: {}", e),
                    context: ErrorContext::new(),
                })
            })?;
        }

        // Log final stats
        if self.config.log_stats {
            if let Some(duration) = stats_clone.duration() {
                let rps = stats_clone.requests_per_second().unwrap_or(0.0);
                info!(
                    "Final stats: {} requests, {} responses, {} items, {} errors, {:.2} req/s, {:.2}s elapsed",
                    stats_clone.request_count,
                    stats_clone.response_count,
                    stats_clone.item_count,
                    stats_clone.error_count,
                    rps,
                    duration.as_secs_f64(),
                );
            }
        }

        // Get error statistics
        let error_stats = self.error_manager.stats().await;
        info!(
            "Error stats: {} total errors, {} retries, {} aborts, {} skips, {} ignored",
            error_stats.total_errors,
            error_stats.retries,
            error_stats.aborts,
            error_stats.skips,
            error_stats.ignored
        );

        // Create a clone of signals for use at the end of the function
        let signals_clone = self.signals.clone();

        // Use the cloned signals at the end of the function
        signals_clone
            .send_catch_log(Signal::EngineStopped, SignalArgs::None)
            .await;

        // Stop resource monitoring
        if let Some(ref controller) = self.resource_controller {
            controller.stop().await;
        }

        Ok(stats_clone)
    }

    /// Get the current engine statistics
    pub async fn stats(&self) -> EngineStats {
        self.stats.read().await.clone()
    }

    /// Check if the engine is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Pause the engine
    pub async fn pause(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if !*running {
            return Err(Box::new(Error::Other {
                message: "Engine is not running".to_string(),
                context: ErrorContext::new(),
            }));
        }
        *running = false;
        info!("Engine paused");

        // Send pause signal
        self.signals
            .send_catch_log(Signal::EnginePaused, SignalArgs::None)
            .await;

        Ok(())
    }

    /// Resume the engine
    pub async fn resume(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(Box::new(Error::Other {
                message: "Engine is already running".to_string(),
                context: ErrorContext::new(),
            }));
        }
        *running = true;
        info!("Engine resumed");

        // Notify all waiting tasks to resume execution
        self.pause_notify.notify_waiters();

        // Send resume signal
        self.signals
            .send_catch_log(Signal::EngineResumed, SignalArgs::None)
            .await;

        Ok(())
    }

    /// Save engine state to file
    pub async fn save_state(&self, path: &str) -> Result<()> {
        // Get current state
        let mut stats = self.stats.read().await.clone();
        stats.prepare_for_serialization();

        let state = EngineState {
            stats,
            // More state can be added here
        };

        // Serialize state
        let serialized = serde_json::to_string_pretty(&state).map_err(|e| {
            Box::new(Error::Other {
                message: format!("Failed to serialize engine state: {}", e),
                context: ErrorContext::new(),
            })
        })?;

        // Write to file
        tokio::fs::write(path, serialized).await.map_err(|e| {
            Box::new(Error::Other {
                message: format!("Failed to write engine state to file: {}", e),
                context: ErrorContext::new(),
            })
        })?;

        info!("Engine state saved to {}", path);
        Ok(())
    }

    /// Load engine state from file
    pub async fn load_state(&self, path: &str) -> Result<()> {
        // Read file
        let serialized = tokio::fs::read_to_string(path).await.map_err(|e| {
            Box::new(Error::Other {
                message: format!("Failed to read engine state from file: {}", e),
                context: ErrorContext::new(),
            })
        })?;

        // Deserialize state
        let mut state: EngineState = serde_json::from_str(&serialized).map_err(|e| {
            Box::new(Error::Other {
                message: format!("Failed to deserialize engine state: {}", e),
                context: ErrorContext::new(),
            })
        })?;

        // Restore state
        state.stats.after_deserialization();
        let mut stats = self.stats.write().await;
        *stats = state.stats;
        // More state can be restored here

        info!("Engine state loaded from {}", path);
        Ok(())
    }

    /// Get the error manager
    pub fn error_manager(&self) -> Arc<ErrorManager> {
        self.error_manager.clone()
    }

    /// Set a custom error manager
    pub fn with_error_manager(mut self, error_manager: Arc<ErrorManager>) -> Self {
        self.error_manager = error_manager;
        self
    }

    /// Get current resource stats
    pub async fn get_resource_stats(&self) -> Option<ResourceStats> {
        if let Some(ref controller) = self.resource_controller {
            Some(controller.get_stats().await)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scrapy_rs_core::item::DynamicItem;
    use scrapy_rs_core::request::Request;
    use scrapy_rs_core::response::Response;
    use scrapy_rs_core::spider::ParseOutput;
    use scrapy_rs_core::spider::Spider;
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct TestSpider {
        name: String,
        start_urls: Vec<String>,
    }

    #[scrapy_rs_core::async_trait]
    impl Spider for TestSpider {
        fn name(&self) -> &str {
            &self.name
        }

        fn start_urls(&self) -> Vec<String> {
            self.start_urls.clone()
        }

        async fn parse(&self, response: Response) -> Result<ParseOutput> {
            let mut output = ParseOutput::new();

            // Extract a simple item
            let mut item = DynamicItem::new("test");
            item.set("url", response.url.to_string());
            item.set("status", response.status as u64);

            output.add_item(item);

            // Follow a link if on the first page
            if response.url.path() == "/page1" {
                if let Ok(request) = Request::get(format!(
                    "{}/page2",
                    response.url.origin().ascii_serialization()
                )) {
                    output.add_request(request);
                }
            }

            Ok(output)
        }
    }

    #[tokio::test]
    async fn test_engine_run() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Mock responses
        Mock::given(method("GET"))
            .and(path("/page1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Page 1"))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/page2"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Page 2"))
            .mount(&mock_server)
            .await;

        // Create a test spider
        let spider = Arc::new(TestSpider {
            name: "test_spider".to_string(),
            start_urls: vec![format!("{}/page1", mock_server.uri())],
        });

        // Create an engine with a small concurrency limit
        let mut engine = Engine::new(spider).unwrap();
        engine = engine.with_config(EngineConfig {
            concurrent_requests: 2,
            log_stats: false,
            ..EngineConfig::default()
        });

        // Run the engine
        let stats = engine.run().await.unwrap();

        // Check the stats
        assert_eq!(stats.request_count, 2);
        assert_eq!(stats.response_count, 2);
        assert_eq!(stats.item_count, 2);
        assert_eq!(stats.error_count, 0);
    }

    pub mod error_handling;
    pub mod resource_control_test;
}

mod mock;
