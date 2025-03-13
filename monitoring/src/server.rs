use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{State, WebSocketUpgrade},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info};

use crate::metrics::MetricsCollector;
use crate::MonitoringEvent;
use crate::Result;

/// Monitoring server for RS-Spider
pub struct MonitoringServer {
    /// Port to listen on
    port: u16,
    /// Metrics collector
    metrics_collector: Arc<MetricsCollector>,
    /// Event sender
    event_tx: broadcast::Sender<MonitoringEvent>,
}

impl MonitoringServer {
    /// Create a new monitoring server
    pub fn new(port: u16, metrics_collector: Arc<MetricsCollector>, event_tx: broadcast::Sender<MonitoringEvent>) -> Self {
        Self {
            port,
            metrics_collector,
            event_tx,
        }
    }
    
    /// Start the monitoring server
    pub async fn start(&self) -> crate::Result<JoinHandle<()>> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let metrics_collector = self.metrics_collector.clone();
        let event_tx = self.event_tx.clone();
        
        // Initialize tracing
        tracing_subscriber::fmt()
            .with_env_filter("info")
            .init();
        
        // Create app state
        let state = AppState {
            metrics_collector,
            event_tx,
        };
        
        // Build router
        let app = Router::new()
            .route("/", get(|| async { "RS-Spider Monitoring Server" }))
            .route("/metrics", get(metrics_handler))
            .route("/events", get(events_handler))
            .route("/ws", get(ws_handler))
            .with_state(state)
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive());
        
        // Start server
        info!("Starting monitoring server on {}", addr);
        
        let handle = tokio::spawn(async move {
            let server = axum::Server::bind(&addr)
                .serve(app.into_make_service());
                
            if let Err(e) = server.await {
                error!("Server error: {}", e);
            }
        });
        
        Ok(handle)
    }
}

/// Metrics server for RS-Spider
pub struct MetricsServer {
    /// Port to listen on
    port: u16,
    /// Metrics collector
    metrics_collector: Arc<MetricsCollector>,
}

impl MetricsServer {
    /// Create a new metrics server
    pub fn new(port: u16, metrics_collector: Arc<MetricsCollector>) -> Self {
        Self {
            port,
            metrics_collector,
        }
    }
    
    /// Start the metrics server
    pub async fn start(&self) -> crate::Result<JoinHandle<()>> {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let metrics_collector = self.metrics_collector.clone();
        
        // Initialize tracing
        tracing_subscriber::fmt()
            .with_env_filter("info")
            .init();
        
        // Create app state
        let state = AppState {
            metrics_collector,
            event_tx: broadcast::channel(10).0,
        };
        
        // Build router
        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .with_state(state)
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive());
        
        // Start server
        info!("Starting metrics server on {}", addr);
        
        let handle = tokio::spawn(async move {
            let server = axum::Server::bind(&addr)
                .serve(app.into_make_service());
                
            if let Err(e) = server.await {
                error!("Server error: {}", e);
            }
        });
        
        Ok(handle)
    }
}

/// App state for the monitoring server
#[derive(Clone)]
struct AppState {
    /// Metrics collector
    metrics_collector: Arc<MetricsCollector>,
    /// Event sender
    event_tx: broadcast::Sender<MonitoringEvent>,
}

/// Metrics handler
async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let metrics = state.metrics_collector.get_metrics().await;
    
    // Convert metrics to JSON
    match serde_json::to_string(&metrics) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => {
            error!("Failed to serialize metrics: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize metrics").into_response()
        }
    }
}

/// Events handler
async fn events_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut rx = state.event_tx.subscribe();
    
    // Create a stream of events
    let stream = async_stream::stream! {
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                yield json;
            }
        }
    };
    
    // Return the stream as a response
    axum::response::sse::Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
}

/// WebSocket handler
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.event_tx.subscribe();
    
    // Task to send events to the client
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });
    
    // Task to receive messages from the client
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    debug!("Received message: {}", text);
                    // Handle client messages if needed
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
} 