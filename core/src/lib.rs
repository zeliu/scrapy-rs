pub mod error;
pub mod error_handler;
pub mod item;
pub mod request;
pub mod response;
pub mod signal;
pub mod spider;

pub use error::{Error, ErrorContext, HttpError, NetworkError, ResponseParseError, Result};
pub use error_handler::{
    ErrorAction, ErrorCallback, ErrorHandler, ErrorManager, ErrorStats,
    DefaultErrorHandler, LogErrorCallback, IgnoreErrorCallback, AbortOnErrorCallback,
};
pub use item::Item;
pub use request::Request;
pub use response::Response;
pub use signal::{Signal, SignalArgs, SignalManager};
pub use spider::Spider;

/// Re-export commonly used crates
pub use async_trait::async_trait;
pub use futures;
pub use serde;
pub use serde_json;
pub use url;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
} 