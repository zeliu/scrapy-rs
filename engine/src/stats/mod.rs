// Engine statistics and state

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

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
}

impl EngineStats {
    /// Calculate the duration of the crawl
    pub fn duration(&self) -> Option<Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            (Some(start), None) => Some(Instant::now().duration_since(start)),
            _ => None,
        }
    }

    /// Calculate the requests per second
    pub fn requests_per_second(&self) -> Option<f64> {
        self.duration().map(|duration| {
            let seconds = duration.as_secs_f64();
            if seconds > 0.0 {
                self.request_count as f64 / seconds
            } else {
                0.0
            }
        })
    }

    /// Prepare the stats for serialization by removing non-serializable fields
    pub fn prepare_for_serialization(&mut self) {
        self.start_time = None;
        self.end_time = None;
    }

    /// Restore the stats after deserialization
    pub fn after_deserialization(&mut self) {
        self.start_time = Some(Instant::now());
        self.end_time = None;
    }
}

/// Engine state for serialization and persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineState {
    /// Engine statistics
    pub stats: EngineStats,
    // More state can be added here
}
