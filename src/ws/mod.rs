//! Kaiheila websocket protocol client implement

mod client;
mod event;
pub(crate) mod message;

pub use client::{
    Client, ConnectGatewayError, EventStream, EventStreamError, EventStreamErrorKind, RunError,
    WaitHelloError,
};
pub use event::Event;
