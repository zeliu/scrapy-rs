use log::{debug, info, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Resource usage statistics
#[derive(Debug, Clone, Default)]
pub struct ResourceStats {
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// CPU usage percentage (0-100)
    pub cpu_usage: f32,
    /// Number of active tasks
    pub active_tasks: usize,
    /// Number of pending requests
    pub pending_requests: usize,
    /// Timestamp of the last update
    pub last_update: Option<Instant>,
}

/// Resource limits configuration
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory usage in bytes (0 = no limit)
    pub max_memory: u64,
    /// Maximum CPU usage percentage (0 = no limit)
    pub max_cpu: f32,
    /// Maximum number of active tasks (0 = no limit)
    pub max_tasks: usize,
    /// Maximum number of pending requests (0 = no limit)
    pub max_pending_requests: usize,
    /// Throttling factor when limits are reached (0.0-1.0)
    pub throttle_factor: f32,
    /// Monitoring interval in milliseconds
    pub monitor_interval_ms: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 0,           // No limit by default
            max_cpu: 0.0,            // No limit by default
            max_tasks: 0,            // No limit by default
            max_pending_requests: 0, // No limit by default
            throttle_factor: 0.5,
            monitor_interval_ms: 1000,
        }
    }
}

/// Resource controller for managing system resources
pub struct ResourceController {
    /// System information
    #[allow(dead_code)]
    system: System,
    /// Current process ID
    pid: u32,
    /// Resource usage statistics
    stats: Arc<RwLock<ResourceStats>>,
    /// Resource limits
    limits: ResourceLimits,
    /// Whether the controller is running
    running: Arc<RwLock<bool>>,
}

impl ResourceController {
    /// Create a new resource controller
    pub fn new(limits: ResourceLimits) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system,
            pid: std::process::id(),
            stats: Arc::new(RwLock::new(ResourceStats::default())),
            limits,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the resource monitoring
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            return;
        }
        *running = true;
        drop(running);

        let stats = self.stats.clone();
        let limits = self.limits.clone();
        let running = self.running.clone();
        let pid = self.pid;
        let mut system = System::new_all();

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_millis(limits.monitor_interval_ms));

            while *running.read().await {
                // Wait for the next interval using tokio's interval instead of sleep for better scheduling
                interval.tick().await;

                // Refresh system information
                system.refresh_all();

                // Yield to the scheduler occasionally to ensure fairness
                tokio::task::yield_now().await;

                // Get process information
                if let Some(process) = system.process(Pid::from_u32(pid)) {
                    let memory_usage = process.memory();
                    let cpu_usage = process.cpu_usage();

                    // Optimization: Only lock stats when necessary, and minimize lock holding time
                    let (active_tasks, pending_requests) = {
                        let stats_read = stats.read().await;
                        (stats_read.active_tasks, stats_read.pending_requests)
                    };

                    // Check if throttling needs to be applied, without locking stats again
                    let throttle = (limits.max_memory > 0 && memory_usage > limits.max_memory)
                        || (limits.max_cpu > 0.0 && cpu_usage > limits.max_cpu)
                        || (limits.max_tasks > 0 && active_tasks > limits.max_tasks)
                        || (limits.max_pending_requests > 0
                            && pending_requests > limits.max_pending_requests);

                    // Update stats fields
                    {
                        let mut stats_write = stats.write().await;
                        stats_write.memory_usage = memory_usage;
                        stats_write.cpu_usage = cpu_usage;
                        stats_write.last_update = Some(Instant::now());
                    }

                    if throttle {
                        // Log throttling
                        warn!("Resource limits exceeded: memory={}MB, CPU={}%, tasks={}, requests={}. Throttling...",
                              memory_usage / 1024 / 1024, cpu_usage, active_tasks, pending_requests);

                        // Sleep to throttle
                        let throttle_ms =
                            (limits.monitor_interval_ms as f32 * limits.throttle_factor) as u64;
                        tokio::time::sleep(Duration::from_millis(throttle_ms)).await;
                    }
                }
            }
        });

        info!("Resource controller started");
    }

    /// Stop the resource monitoring
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Resource controller stopped");
    }

    /// Update active tasks count
    pub async fn update_active_tasks(&self, count: usize) {
        let mut stats = self.stats.write().await;
        stats.active_tasks = count;
    }

    /// Update pending requests count
    pub async fn update_pending_requests(&self, count: usize) {
        let mut stats = self.stats.write().await;
        stats.pending_requests = count;
    }

    /// Get current resource stats
    pub async fn get_stats(&self) -> ResourceStats {
        self.stats.read().await.clone()
    }

    /// Check if throttling is needed
    pub async fn should_throttle(&self) -> bool {
        let stats = self.stats.read().await;
        let limits = &self.limits;

        (limits.max_memory > 0 && stats.memory_usage > limits.max_memory)
            || (limits.max_cpu > 0.0 && stats.cpu_usage > limits.max_cpu)
            || (limits.max_tasks > 0 && stats.active_tasks > limits.max_tasks)
            || (limits.max_pending_requests > 0
                && stats.pending_requests > limits.max_pending_requests)
    }

    /// Apply throttling if needed
    pub async fn throttle_if_needed(&self) {
        if self.should_throttle().await {
            let throttle_ms =
                (self.limits.monitor_interval_ms as f32 * self.limits.throttle_factor) as u64;
            debug!("Throttling for {}ms", throttle_ms);
            sleep(Duration::from_millis(throttle_ms)).await;

            // add yield to ensure fair scheduling
            tokio::task::yield_now().await;
        }
    }
}
