use std::task::Poll;

use futures_util::Stream;
use snafu::prelude::*;
use tokio::sync::mpsc;

use super::super::ConnectGatewayError;
use crate::{
    api::types::GatewayResumeArguments,
    ws::{client::WaitHelloError, message::MessageStreamSinkError, Event},
};

/// Error for event stream
#[derive(Debug, Snafu)]
#[snafu(display("event stream broken: {source}"))]
pub struct EventStreamError {
    /// arguments for conversion resume
    pub resume: GatewayResumeArguments,
    /// real error
    pub source: EventStreamErrorKind,
}

/// Error kind for event stream
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), module(error), context(suffix(false)))]
pub enum EventStreamErrorKind {
    /// underlying message stream broken
    #[snafu(display("underlying message stream broken: {source}"))]
    MessageStream {
        /// source error
        #[snafu(source(from(MessageStreamSinkError, Box::new)))]
        source: Box<dyn std::error::Error + Send>,
    },

    /// received server reconnect message
    #[snafu(display("received server reconnect request, code {code}, message: {message}"))]
    Reconnect {
        /// reconnect reason code
        /// see: <https://developer.kaiheila.cn/doc/websocket#%E4%BF%A1%E4%BB%A4[5]%20RECONNECT>
        code: i64,
        /// reconnect reason message
        message: String,
    },

    /// reconnect to websocket gateway failed
    #[snafu(display("(re)connect ws gateway failed: {source}"))]
    ReConnectGatewayFailed {
        /// source error
        source: ConnectGatewayError,
    },

    /// reconnect to websocket gateway failed
    #[snafu(display("(re)wait hello from ws gateway failed: {source}"))]
    ReWaitHelloFailed {
        /// source error
        source: WaitHelloError,
    },
}

/// Kaiheila websocket event stream
#[derive(Debug)]
pub struct EventStream {
    pub(crate) rx: mpsc::Receiver<Result<Box<Event>, EventStreamError>>,
}

impl Stream for EventStream {
    type Item = Result<Box<Event>, EventStreamError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}
