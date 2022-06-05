//! kaiheila api

mod client;
mod error;
pub mod types;

pub use client::Client;
pub use error::Error;

/// Result type for api module
pub type Result<T> = std::result::Result<T, Error>;
