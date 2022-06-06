//! # Burz
//!
//! A Kaiheila bot framework.

#![deny(warnings)]
#![deny(clippy::all)]
#![deny(missing_debug_implementations, missing_docs)]
#![forbid(unsafe_code)]

pub mod api;
pub mod ws;

mod bot;
mod error;

pub use bot::Bot;
pub use error::{Error, Result};
