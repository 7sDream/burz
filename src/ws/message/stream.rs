use std::task::Poll;

use bytes::Bytes;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use snafu::prelude::*;
use tokio_tungstenite::tungstenite as websocket;

use super::{Message, ParseMessageError};
use crate::ws::client::WebsocketClient;

/// Error when read/write message stream/sink
#[derive(Debug, Snafu)]
#[snafu(module(error), context(suffix(false)))]
pub enum MessageStreamSinkError {
    /// underlying websocket stream broken
    #[snafu(display("underlying websocket stream broken: {source}"))]
    Websocket {
        /// source error
        source: websocket::Error,
    },

    /// received an non-binary frame
    #[snafu(display("received a non-binary type frame"))]
    NotBinaryFrame,

    /// parse binary message data failed
    #[snafu(display("parse frame to message failed: {source}"))]
    ParseMessageFailed {
        /// source error
        source: ParseMessageError,
    },
}

impl MessageStreamSinkError {
    /// Check if this error will make the stream/sink stop
    pub fn is_fatal(&self) -> bool {
        match self {
            Self::Websocket { .. } => true,
            Self::NotBinaryFrame => false,
            Self::ParseMessageFailed { source } => {
                !matches!(source, ParseMessageError::UnknownMessageType { .. })
            }
        }
    }
}

/// Kaiheila websocket message stream/sink
#[derive(Debug)]
pub struct MessageStreamSink {
    ws: WebsocketClient,
    compress: bool,
}

impl MessageStreamSink {
    /// Construct a new stream with underlying websocket connection.
    ///
    /// the `compress` argument controls if the stream will decompress binary data
    /// before parse it to message.
    pub fn new(ws: WebsocketClient, compress: bool) -> Self {
        Self { ws, compress }
    }
}

impl Stream for MessageStreamSink {
    type Item = Result<Message, MessageStreamSinkError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.ws.poll_next_unpin(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(frame) => {
                let frame = frame.unwrap().context(error::Websocket)?;
                let result = match frame {
                    websocket::Message::Binary(data) => {
                        let buffer: Bytes = data.into();
                        match Message::decode(buffer.clone(), self.compress) {
                            Ok(msg) => Ok(msg),
                            Err(e) => {
                                log::trace!(
                                    "Parse failed message data: {}",
                                    std::str::from_utf8(&buffer).unwrap_or("<not-utf8-binary>")
                                );
                                Err(MessageStreamSinkError::ParseMessageFailed { source: e })
                            }
                        }
                    }
                    _ => Err(MessageStreamSinkError::NotBinaryFrame),
                };
                Poll::Ready(Some(result))
            }
        }
    }
}

impl Sink<Message> for MessageStreamSink {
    type Error = MessageStreamSinkError;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.ws
            .poll_ready_unpin(cx)
            .map_err(|e| Self::Error::Websocket { source: e })
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        self.ws
            .start_send_unpin(websocket::Message::Binary(item.encode()))
            .map_err(|e| Self::Error::Websocket { source: e })
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.ws
            .poll_flush_unpin(cx)
            .map_err(|e| Self::Error::Websocket { source: e })
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        self.ws
            .poll_close_unpin(cx)
            .map_err(|e| Self::Error::Websocket { source: e })
    }
}
