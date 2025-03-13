use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::Result;
use crate::item::DynamicItem;
use crate::request::Request;
use crate::response::Response;
use crate::spider::Spider;

/// Define signal types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Signal {
    /// Sent when engine starts
    EngineStarted,
    /// Sent when engine stops
    EngineStopped,
    /// Sent when engine pauses
    EnginePaused,
    /// Sent when engine resumes
    EngineResumed,
    /// Sent when spider opens
    SpiderOpened,
    /// Sent when spider closes
    SpiderClosed,
    /// Sent before request is scheduled
    RequestScheduled,
    /// Sent after request is sent
    RequestSent,
    /// Sent after response is received
    ResponseReceived,
    /// Sent after item is scraped
    ItemScraped,
    /// Sent when error occurs
    ErrorOccurred,
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Signal::EngineStarted => write!(f, "engine_started"),
            Signal::EngineStopped => write!(f, "engine_stopped"),
            Signal::EnginePaused => write!(f, "engine_paused"),
            Signal::EngineResumed => write!(f, "engine_resumed"),
            Signal::SpiderOpened => write!(f, "spider_opened"),
            Signal::SpiderClosed => write!(f, "spider_closed"),
            Signal::RequestScheduled => write!(f, "request_scheduled"),
            Signal::RequestSent => write!(f, "request_sent"),
            Signal::ResponseReceived => write!(f, "response_received"),
            Signal::ItemScraped => write!(f, "item_scraped"),
            Signal::ErrorOccurred => write!(f, "error_occurred"),
        }
    }
}

/// Signal arguments
#[derive(Clone)]
pub enum SignalArgs {
    /// No arguments
    None,
    /// Spider related
    Spider(Arc<dyn Spider>),
    /// Request related
    Request(Request),
    /// Response related
    Response(Response),
    /// Item related
    Item(DynamicItem),
    /// Error related
    Error(String),
    /// Custom arguments
    Custom(serde_json::Value),
}

impl std::fmt::Debug for SignalArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "SignalArgs::None"),
            Self::Spider(spider) => write!(f, "SignalArgs::Spider({})", spider.name()),
            Self::Request(request) => write!(f, "SignalArgs::Request({:?})", request),
            Self::Response(response) => write!(f, "SignalArgs::Response({:?})", response),
            Self::Item(item) => write!(f, "SignalArgs::Item({:?})", item),
            Self::Error(error) => write!(f, "SignalArgs::Error({:?})", error),
            Self::Custom(value) => write!(f, "SignalArgs::Custom({:?})", value),
        }
    }
}

/// Signal handler type
pub type SignalHandler = Box<dyn Fn(SignalArgs) -> Result<()> + Send + Sync + 'static>;

/// Signal manager
pub struct SignalManager {
    /// Signal handler mapping
    handlers: Arc<RwLock<HashMap<Signal, Vec<SignalHandler>>>>,
}

impl SignalManager {
    /// Create a new signal manager
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Connect signal handler
    pub async fn connect<F>(&self, signal: Signal, handler: F) -> Result<()>
    where
        F: Fn(SignalArgs) -> Result<()> + Send + Sync + 'static,
    {
        let mut handlers = self.handlers.write().await;
        handlers
            .entry(signal)
            .or_insert_with(Vec::new)
            .push(Box::new(handler));
        Ok(())
    }

    /// Send signal
    pub async fn send(&self, signal: Signal, args: SignalArgs) -> Result<()> {
        let handlers = self.handlers.read().await;
        if let Some(handlers) = handlers.get(&signal) {
            for handler in handlers {
                handler(args.clone())?;
            }
        }
        Ok(())
    }

    /// Send signal and catch errors
    pub async fn send_catch_log(&self, signal: Signal, args: SignalArgs) {
        if let Err(e) = self.send(signal, args.clone()).await {
            log::error!("Error sending signal {}: {}", signal, e);
        }
    }

    /// Disconnect all signal handlers
    pub async fn disconnect_all(&self) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.clear();
        Ok(())
    }

    /// Disconnect all handlers for a specific signal
    pub async fn disconnect(&self, signal: Signal) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.remove(&signal);
        Ok(())
    }
}

impl Default for SignalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn test_signal_manager() {
        let signal_manager = SignalManager::new();
        let called = Arc::new(AtomicBool::new(false));
        
        let called_clone = called.clone();
        signal_manager
            .connect(Signal::EngineStarted, move |_| {
                called_clone.store(true, Ordering::SeqCst);
                Ok(())
            })
            .await
            .unwrap();
        
        signal_manager
            .send(Signal::EngineStarted, SignalArgs::None)
            .await
            .unwrap();
        
        assert!(called.load(Ordering::SeqCst));
    }
    
    #[tokio::test]
    async fn test_multiple_handlers() {
        let signal_manager = SignalManager::new();
        let counter1 = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter2 = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        
        let counter1_clone = counter1.clone();
        signal_manager
            .connect(Signal::ItemScraped, move |_| {
                counter1_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await
            .unwrap();
        
        let counter2_clone = counter2.clone();
        signal_manager
            .connect(Signal::ItemScraped, move |_| {
                counter2_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await
            .unwrap();
        
        signal_manager
            .send(Signal::ItemScraped, SignalArgs::None)
            .await
            .unwrap();
        
        assert_eq!(counter1.load(Ordering::SeqCst), 1);
        assert_eq!(counter2.load(Ordering::SeqCst), 1);
    }
    
    #[tokio::test]
    async fn test_disconnect() {
        let signal_manager = SignalManager::new();
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        
        let counter_clone = counter.clone();
        signal_manager
            .connect(Signal::RequestSent, move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await
            .unwrap();
        
        signal_manager
            .send(Signal::RequestSent, SignalArgs::None)
            .await
            .unwrap();
        
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        
        signal_manager.disconnect(Signal::RequestSent).await.unwrap();
        
        signal_manager
            .send(Signal::RequestSent, SignalArgs::None)
            .await
            .unwrap();
        
        // Counter should not increase because handler is disconnected
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
} 