//! framework error types

use snafu::prelude::*;

use super::api::Error as APIError;
use super::ws::client::RunError;

/// framework result type
pub type Result<T> = std::result::Result<T, Error>;

/// framework error type
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(false)))]
pub enum Error {
    /// Call kaiheila api failed
    #[snafu(display("call kaiheila api failed: {source}"))]
    CallAPIFailed {
        /// source error
        source: APIError,
    },

    /// Received invalid websocket gateway url address
    #[snafu(display("invalid gateway url {url}"))]
    InvalidGatewayURL {
        /// received url
        url: String,
        /// source error
        source: crate::api::types::ParseGatewayURLError,
    },

    /// Run websocket client failed
    #[snafu(display("run inner websocket client failed: {source}"))]
    RunWebsocketClientFailed {
        /// source error
        source: RunError,
    },
}
