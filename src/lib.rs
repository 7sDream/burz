//! # Burz
//!
//! A Kaiheila bot framework.

#![deny(warnings)]
#![deny(clippy::all)]
#![deny(missing_debug_implementations, missing_docs)]
#![forbid(unsafe_code)]

pub mod api;
pub mod filter;
pub mod ws;

mod bot;
mod error;
mod subscriber;

pub use bot::Bot;
pub use error::{Error, Result};
pub use filter::{Filter, FilterExt};
pub use subscriber::Subscriber;
