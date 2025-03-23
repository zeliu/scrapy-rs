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
use scrapy_rs_scheduler::Scheduler;
use tokio::sync::{mpsc, Notify, RwLock};
use tokio::time::sleep;

// Make these modules public
pub mod config;
pub mod resource_control;
pub mod slot;
pub mod stats;
pub mod utils;

// Re-export key types
pub use config::{EngineConfig, SchedulerType};
pub use resource_control::{ResourceController, ResourceLimits, ResourceStats};
pub use slot::{Slot, SlotManager};
pub use stats::{EngineState, EngineStats};

// Import utility functions
use crate::slot::create_request_id;
use crate::utils::create_scheduler;

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

    /// Slot manager for controlling domain-specific request processing
    slot_manager: Arc<SlotManager>,

    /// Last request time per domain for rate limiting
    #[allow(dead_code)]
    domain_last_request: Arc<RwLock<HashMap<String, Instant>>>,
}

impl Engine {
    /// Create a new engine with the given spider and default components
    pub fn new(spider: Arc<dyn Spider>) -> Result<Self> {
        let config = EngineConfig::default();

        // Create the scheduler based on the configuration
        let scheduler = create_scheduler(&config)?;

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

        // Create the slot manager
        let slot_manager = Arc::new(SlotManager::new(
            config.max_active_size_per_domain,
            config.delay_per_domain,
        ));
        // Create the last request time per domain for rate limiting
        let domain_last_request = Arc::new(RwLock::new(HashMap::new()));

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
            slot_manager,
            domain_last_request,
        })
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

        // Create the slot manager
        let slot_manager = Arc::new(SlotManager::new(
            config.max_active_size_per_domain,
            config.delay_per_domain,
        ));

        // Create the last request time per domain for rate limiting
        let domain_last_request = Arc::new(RwLock::new(HashMap::new()));

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
            slot_manager,

            domain_last_request,
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
        let _stats_task = if self.config.log_stats {
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
                        .process_request(request.clone(), &*self.spider)
                        .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            // Request middleware rejected the request
                            debug!("Request middleware rejected request: {}", e);
                            let mut stats = self.stats.write().await;
                            stats.error_count += 1;

                            // Store URL before moving the request
                            let url = request.url.as_str().to_string();

                            // Handle error from middleware
                            let error_with_context =
                                Error::other(format!("Request middleware rejected request: {}", e))
                                    .with_component("request_middleware")
                                    .with_url(url)
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
                                        "Error downloading request: {}",
                                        &e.to_string()
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

                    // Get the domain from the request
                    let domain = processed_request
                        .url
                        .host_str()
                        .unwrap_or("unknown")
                        .to_string();

                    // Get or create the slot for this domain
                    let slot = self.slot_manager.get_slot(&domain).await;

                    // Check if the slot needs throttling
                    if slot.needs_throttle().await {
                        debug!("Throttling requests for domain: {}", domain);
                        sleep(Duration::from_millis(self.config.download_delay_ms)).await;
                    }

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
                    let slot_clone = slot.clone();

                    // Add a Notify for pause/resume notification
                    let task_pause_notify = self.pause_notify.clone();

                    let task = tokio::spawn(async move {
                        // Check if paused
                        if !*running.read().await {
                            // Wait for resume notification, instead of polling
                            task_pause_notify.notified().await;
                        }

                        // Add the request to the slot
                        let _response_rx = slot_clone.add_request(processed_request.clone()).await;

                        // Wait for the slot to process the request
                        let (request, _) = match slot_clone.next_request().await {
                            Some(tuple) => tuple,
                            None => {
                                // Slot is closing or no request available
                                warn!(
                                    "Slot for domain {} is closing or no request available",
                                    domain
                                );
                                return;
                            }
                        };

                        // Download the request
                        let download_result = downloader.download(request.clone()).await;
                        // Create a unique identifier for the request from its URL
                        let request_id = create_request_id(&request);

                        // Process the download result
                        let response = match download_result {
                            Ok(resp) => {
                                // Calculate response size
                                let response_size = resp.body.len();

                                // Finish the request in the slot with the actual response size
                                slot_clone
                                    .finish_request(&request_id, Some(response_size))
                                    .await;

                                resp
                            }
                            Err(e) => {
                                // Finish the request in the slot with no response size
                                slot_clone.finish_request(&request_id, None).await;

                                // Update stats
                                {
                                    let mut stats = stats.write().await;
                                    stats.error_count += 1;
                                }

                                error!("Error downloading request: {}", e);

                                // Store the error message
                                let error_message = format!("Error downloading request: {}", e);

                                // Create error with context
                                let error_with_context = e
                                    .with_component("downloader")
                                    .with_url(request.url.as_str().to_string())
                                    .with_spider_name(spider.name());

                                // Handle error
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
                                        SignalArgs::Error(error_message),
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

        // Call our dedicated close method to handle all cleanup properly
        // This ensures consistent cleanup behavior in normal and error scenarios
        self.close().await?;

        // Return the final stats
        let final_stats = self.stats.read().await.clone();
        Ok(final_stats)
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

    /// Close the engine and clean up resources
    ///
    /// This method ensures proper cleanup of all engine resources, including:
    /// - Closing the spider
    /// - Shutting down pipelines
    /// - Stopping middleware
    /// - Canceling active tasks
    /// - Sending appropriate signals
    ///
    /// It can be called at any time, even if the engine is in a hanging state
    /// or when a timeout occurs.
    pub async fn close(&self) -> Result<()> {
        info!("Initiating controlled engine shutdown");

        // Set engine to not running state
        {
            let mut running = self.running.write().await;
            *running = false;
        }

        // Send engine stopping signal
        self.signals
            .send_catch_log(Signal::EngineStopping, SignalArgs::None)
            .await;

        // Close the spider
        info!("Closing spider: {}", self.spider.name());

        // Use timeout mechanism to avoid hanging if any of these operations take too long
        let timeout_duration = Duration::from_secs(5); // 5 second timeout for each operation

        // Close pipelines with timeout
        match tokio::time::timeout(timeout_duration, self.pipelines.close_spider(&*self.spider))
            .await
        {
            Ok(result) => {
                if let Err(e) = result {
                    warn!("Error closing pipelines: {}", e);
                } else {
                    debug!("Pipelines closed successfully");
                }
            }
            Err(_) => warn!("Timeout while closing pipelines"),
        }

        // Close response middlewares with timeout
        match tokio::time::timeout(
            timeout_duration,
            self.response_middlewares.spider_closed(&*self.spider),
        )
        .await
        {
            Ok(result) => {
                if let Err(e) = result {
                    warn!("Error closing response middlewares: {}", e);
                } else {
                    debug!("Response middlewares closed successfully");
                }
            }
            Err(_) => warn!("Timeout while closing response middlewares"),
        }

        // Close request middlewares with timeout
        match tokio::time::timeout(
            timeout_duration,
            self.request_middlewares.spider_closed(&*self.spider),
        )
        .await
        {
            Ok(result) => {
                if let Err(e) = result {
                    warn!("Error closing request middlewares: {}", e);
                } else {
                    debug!("Request middlewares closed successfully");
                }
            }
            Err(_) => warn!("Timeout while closing request middlewares"),
        }

        // Call spider's closed method with timeout
        match tokio::time::timeout(timeout_duration, self.spider.closed()).await {
            Ok(result) => {
                if let Err(e) = result {
                    warn!("Error calling spider.closed(): {}", e);
                } else {
                    debug!("Spider closed successfully");
                }
            }
            Err(_) => warn!("Timeout while closing spider"),
        }

        // Send spider closed signal
        self.signals
            .send_catch_log(
                Signal::SpiderClosed,
                SignalArgs::Spider(self.spider.clone()),
            )
            .await;

        // Close item channel if it exists
        if let Some(tx) = &self.item_tx {
            // Check if the channel is closed; if not, drop it
            if !tx.is_closed() {
                let _ = tx;
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.end_time = Some(Instant::now());
        }

        // Log final stats
        if self.config.log_stats {
            let stats_clone = self.stats.read().await.clone();
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

        // Notify all waiting tasks to resume execution
        // (Important to unblock any waiting tasks so they can terminate)
        self.pause_notify.notify_waiters();

        // Stop resource monitoring if enabled
        if let Some(ref controller) = self.resource_controller {
            controller.stop().await;
        }

        // Send final engine stopped signal
        self.signals
            .send_catch_log(Signal::EngineStopped, SignalArgs::None)
            .await;

        info!("Engine shutdown completed");
        Ok(())
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

    // Import close_test module
    mod close_test;

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
    pub mod slot_test;
}

mod mock;
