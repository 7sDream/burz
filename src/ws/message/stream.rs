use std::task::Poll;

use futures_util::{Sink, SinkExt, Stream, StreamExt};
use snafu::prelude::*;
use tokio_tungstenite::tungstenite as websocket;

use super::{Message, ParseMessageError};
use crate::ws::client::WebsocketClient;

#[derive(Debug, Snafu)]
#[snafu(module(error), context(suffix(false)))]
pub(crate) enum MessageStreamSinkError {
    #[snafu(display("underlying websocket stream broken: {source}"))]
    Websocket { source: websocket::Error },

    #[snafu(display("received a non-binary type frame"))]
    NotBinaryFrame,

    #[snafu(display("parse frame to message failed: {source}"))]
    ParseMessageFailed { source: ParseMessageError },
}

impl MessageStreamSinkError {
    pub fn need_stop(&self) -> bool {
        match self {
            Self::Websocket { .. } => true,
            Self::NotBinaryFrame => false,
            Self::ParseMessageFailed { source } => {
                !matches!(source, ParseMessageError::UnknownMessageType { .. })
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct MessageStreamSink {
    ws: WebsocketClient,
    compress: bool,
}

impl MessageStreamSink {
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
                        match Message::decode(data.into(), self.compress) {
                            Ok(msg) => Ok(msg),
                            Err(e) => Err(MessageStreamSinkError::ParseMessageFailed { source: e }),
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
