use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::time::sleep;
use warp::{http::HeaderMap, Filter, Rejection, Reply};

/// MockServer configuration
#[derive(Debug, Clone)]
pub struct MockServerConfig {
    /// Port to listen on
    pub port: u16,
    /// Number of pages to generate
    pub page_count: usize,
    /// Links per page
    pub links_per_page: usize,
    /// Response delay in milliseconds
    pub response_delay_ms: u64,
    /// Whether to simulate failures
    pub simulate_failures: bool,
    /// Failure rate (0.0 - 1.0)
    pub failure_rate: f64,
}

impl Default for MockServerConfig {
    fn default() -> Self {
        Self {
            port: 8000,
            page_count: 100,
            links_per_page: 10,
            response_delay_ms: 0,
            simulate_failures: false,
            failure_rate: 0.0,
        }
    }
}

impl MockServerConfig {
    /// Create a new mock server configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the number of pages
    pub fn with_page_count(mut self, count: usize) -> Self {
        self.page_count = count;
        self
    }

    /// Set the number of links per page
    pub fn with_links_per_page(mut self, count: usize) -> Self {
        self.links_per_page = count;
        self
    }

    /// Set the response delay
    pub fn with_response_delay(mut self, delay_ms: u64) -> Self {
        self.response_delay_ms = delay_ms;
        self
    }

    /// Set whether to simulate failures
    pub fn with_simulate_failures(mut self, simulate: bool) -> Self {
        self.simulate_failures = simulate;
        self
    }

    /// Set the failure rate
    pub fn with_failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = rate.clamp(0.0, 1.0); // Clamp between 0 and 1
        self
    }
}

/// State for the mock server
pub struct MockServerState {
    /// Configuration
    pub config: MockServerConfig,
    /// Request counter
    pub request_count: usize,
    /// Headers from all requests
    pub headers: Vec<HashMap<String, String>>,
}

/// Mock server
pub struct MockServer {
    /// Server address
    addr: SocketAddr,
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl MockServer {
    /// Create a new mock server
    pub fn new(config: MockServerConfig) -> Self {
        let addr = ([127, 0, 0, 1], config.port).into();
        Self {
            addr,
            shutdown_tx: None,
        }
    }

    /// Start the server
    pub async fn start(
        &mut self,
        config: MockServerConfig,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let state = Arc::new(Mutex::new(MockServerState {
            config: config.clone(),
            request_count: 0,
            headers: Vec::new(),
        }));

        // Create routes
        let state_filter = warp::any().map(move || Arc::clone(&state));

        // Define the default route (/:id) that serves HTML pages with links
        let page_route = warp::path::param::<usize>()
            .and(warp::get())
            .and(warp::header::headers_cloned())
            .and(state_filter.clone())
            .and_then(handle_page);

        // Define robots.txt route
        let robots_route = warp::path("robots.txt")
            .and(warp::get())
            .and(state_filter.clone())
            .and_then(handle_robots);

        // Define the root route
        let root_route = warp::path::end()
            .and(warp::get())
            .and(warp::header::headers_cloned())
            .and(state_filter.clone())
            .and_then(handle_root);

        // Combine all routes
        let routes = page_route.or(robots_route).or(root_route);

        // Create a shutdown channel
        let (tx, rx) = oneshot::channel();
        self.shutdown_tx = Some(tx);

        // Start the server
        let (_, server) = warp::serve(routes).bind_with_graceful_shutdown(self.addr, async {
            rx.await.ok();
        });

        // Run the server in a separate task
        tokio::spawn(server);

        let base_url = format!("http://{}", self.addr);
        Ok(base_url)
    }

    /// Stop the server
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Get the base URL
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

/// Handle requests for robots.txt
async fn handle_robots(state: Arc<Mutex<MockServerState>>) -> Result<impl Reply, Rejection> {
    let response = if state.lock().unwrap().config.simulate_failures {
        "User-agent: *\nDisallow: /5\nDisallow: /10\nDisallow: /15"
    } else {
        "User-agent: *\nAllow: /"
    };

    // Simulate delay if configured
    let delay_ms = state.lock().unwrap().config.response_delay_ms;
    if delay_ms > 0 {
        sleep(Duration::from_millis(delay_ms)).await;
    }

    Ok(response)
}

/// Handle requests for pages
async fn handle_page(
    id: usize,
    headers: HeaderMap,
    state: Arc<Mutex<MockServerState>>,
) -> Result<impl Reply, Rejection> {
    // Generate page HTML based on ID
    let html = generate_page_html(id, &state);

    // Store request information
    {
        let mut state = state.lock().unwrap();
        state.request_count += 1;

        // Convert headers to a HashMap
        let mut header_map = HashMap::new();
        for (name, value) in headers.iter() {
            if let Ok(value_str) = value.to_str() {
                header_map.insert(name.as_str().to_string(), value_str.to_string());
            }
        }
        state.headers.push(header_map);
    }

    // Check if we should simulate a failure
    let should_fail = {
        let state = state.lock().unwrap();
        state.config.simulate_failures && rand::random::<f64>() < state.config.failure_rate
    };

    if should_fail {
        // Random HTTP error response
        return Err(warp::reject::not_found());
    }

    // Simulate delay if configured
    let delay_ms = state.lock().unwrap().config.response_delay_ms;
    if delay_ms > 0 {
        sleep(Duration::from_millis(delay_ms)).await;
    }

    Ok(html)
}

/// Handle requests for the root
async fn handle_root(
    headers: HeaderMap,
    state: Arc<Mutex<MockServerState>>,
) -> Result<impl Reply, Rejection> {
    // Redirect to the first page
    handle_page(0, headers, state).await
}

/// Generate HTML for a page
fn generate_page_html(id: usize, state: &Arc<Mutex<MockServerState>>) -> String {
    let state = state.lock().unwrap();
    let page_count = state.config.page_count;
    let links_per_page = state.config.links_per_page;

    let mut html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Mock Page {}</title>
    <meta name="description" content="This is a mock page for benchmarking scrapy and scrapy-rs">
</head>
<body>
    <h1>Mock Page {}</h1>
    <p>This is a generated page for benchmarking scrapy and scrapy-rs.</p>
    <p>Current page ID: {}</p>
    <p>Total pages: {}</p>
    <h2>Links</h2>
    <ul>
"#,
        id, id, id, page_count
    );

    // Generate links to other pages
    for i in 0..links_per_page {
        let target_id = (id + i + 1) % page_count;
        html.push_str(&format!(
            r#"        <li><a href="/{0}">Link to Page {0}</a></li>
"#,
            target_id
        ));
    }

    html.push_str(
        r#"    </ul>
    <div class="content">
        <p>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt 
        ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco 
        laboris nisi ut aliquip ex ea commodo consequat.</p>
    </div>
</body>
</html>"#,
    );

    html
}
