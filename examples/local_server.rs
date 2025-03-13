use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use tokio::time::sleep;

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();
    
    // Return different responses based on path
    match path {
        "/delay" => {
            // Delay 5 seconds before returning
            println!("Delaying response for 5 seconds...");
            sleep(Duration::from_secs(5)).await;
            println!("Delay complete, sending response");
            Ok(Response::new(Body::from("Delayed response (5s)")))
        }
        "/delay2" => {
            // Delay 10 seconds before returning
            println!("Delaying response for 10 seconds...");
            sleep(Duration::from_secs(10)).await;
            println!("Delay complete, sending response");
            Ok(Response::new(Body::from("Delayed response (10s)")))
        }
        "/delay3" => {
            // Delay 15 seconds before returning
            println!("Delaying response for 15 seconds...");
            sleep(Duration::from_secs(15)).await;
            println!("Delay complete, sending response");
            Ok(Response::new(Body::from("Delayed response (15s)")))
        }
        "/delay4" => {
            // Delay 20 seconds before returning
            println!("Delaying response for 20 seconds...");
            sleep(Duration::from_secs(20)).await;
            println!("Delay complete, sending response");
            Ok(Response::new(Body::from("Delayed response (20s)")))
        }
        "/delay5" => {
            // Delay 25 seconds before returning
            println!("Delaying response for 25 seconds...");
            sleep(Duration::from_secs(25)).await;
            println!("Delay complete, sending response");
            Ok(Response::new(Body::from("Delayed response (25s)")))
        }
        _ => {
            // Return immediately
            println!("Received request: {}", path);
            Ok(Response::new(Body::from("Hello, World!")))
        }
    }
}

#[tokio::main]
async fn main() {
    // Bind to local address
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    
    // Create service
    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(handle_request))
    });
    
    // Create server
    let server = Server::bind(&addr).serve(make_svc);
    
    // Start server
    println!("Server running on http://{}", addr);
    
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
} 