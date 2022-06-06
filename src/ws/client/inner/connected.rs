use std::{fmt::Debug, time::Duration};

use futures_util::{future, Sink, Stream, StreamExt};
use snafu::prelude::*;
use tokio::time::Instant;

use super::{streaming::ClientStateStreaming, ClientInner, EventStream};
use crate::{
    api::types::GatewayURLInfo,
    ws::{
        client::{inner::streaming::EventStreamSender, WebsocketClient},
        message::{Message, MessageStreamSink, MessageStreamSinkError},
    },
};

/// Error when wait websocket gateway hello message
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), module(error), context(suffix(false)))]
pub enum WaitHelloError {
    /// No message received
    #[snafu(display("timeout when wait server hello message"))]
    Timeout,

    /// underlying message stream broken
    #[snafu(display("underlying message stream broken: {source}"))]
    MessageStream {
        /// source error
        #[snafu(source(from(MessageStreamSinkError, Box::new)))]
        source: Box<dyn std::error::Error + Send>,
    },

    /// received first message is not hello type
    #[snafu(display("received first message is not hello"))]
    MessageNotHello,

    /// hello message code is not zero
    /// see <https://developer.kaiheila.cn/doc/websocket#%E4%BF%A1%E4%BB%A4[1]%20HELLO> for code meaning
    #[snafu(display("hello message code {code} is not zero"))]
    HelloMessageCodeNotZero {
        /// status code
        code: i64,
    },

    /// received hello message has no session id
    #[snafu(display("hello message has no session id"))]
    HelloMessageNoSessionId,
}

#[derive(Debug)]
pub(crate) struct ClientStateConnected {
    pub gateway: GatewayURLInfo,
    pub ws: WebsocketClient,
}

impl ClientInner<ClientStateConnected> {
    async fn real_wait_hello(
        ws: WebsocketClient,
        compress: bool,
    ) -> Result<
        (
            impl Stream<Item = Result<Message, MessageStreamSinkError>>
                + Sink<Message, Error = MessageStreamSinkError>
                + Debug,
            String,
        ),
        WaitHelloError,
    > {
        let mut message_stream = MessageStreamSink::new(ws, compress).filter(|result| {
            let skip = matches!(result, Err(e) if !e.is_fatal());
            if skip {
                log::warn!(
                    "Message stream error happened but ignored: {}",
                    result.as_ref().unwrap_err()
                );
            }
            future::ready(!skip)
        });

        let deadline = Instant::now() + Duration::from_secs(6);

        log::debug!("Waiting hello message, timeout tick: {:?}", deadline);

        let message = tokio::select! {
            _ = tokio::time::sleep_until(deadline) => {
                log::warn!("Wait hello timeout");
                return error::Timeout.fail();
            }
            result = message_stream.next() => {
                result.unwrap().context(error::MessageStream)?
            }
        };

        log::debug!("Wait hello get a {} message", message.type_name());

        ensure!(matches!(message, Message::Hello(_)), error::MessageNotHello,);

        let hello = message.into_hello().unwrap(); // checked in last line

        log::debug!("Hello message data: {:?}", hello);

        ensure!(
            hello.data.code == 0,
            error::HelloMessageCodeNotZero {
                code: hello.data.code
            }
        );

        let session_id = hello
            .data
            .session_id
            .ok_or_else(|| error::HelloMessageNoSessionId.build())?;

        Ok((message_stream, session_id))
    }

    pub async fn wait_hello(mut self) -> Result<EventStream, WaitHelloError> {
        let (message_stream, session_id) =
            Self::real_wait_hello(self.state.ws, self.state.gateway.compress).await?;

        let mut resume = self.state.gateway.resume.take().unwrap_or_default();
        resume.session_id = session_id;

        log::debug!("New resume argument: {:?}", resume);

        let (sink, stream) = message_stream.split();
        let (sender, event_stream) = EventStreamSender::new(resume);

        log::debug!("Move to streaming state");

        ClientInner {
            state: ClientStateStreaming {
                gateway: self.state.gateway,
                sender,
                sink,
                stream,
            },
        }
        .streaming_start();

        Ok(event_stream)
    }

    pub async fn re_wait_hello(mut self, sender: EventStreamSender) {
        let (message_stream, session_id) =
            match Self::real_wait_hello(self.state.ws, self.state.gateway.compress)
                .await
                .context(super::streaming::error::ReWaitHelloFailed)
            {
                Ok((m, s)) => (m, s),
                Err(err) => {
                    log::warn!(
                        "Reconnect state wait hello failed: {}, send event stream error and stop",
                        err
                    );

                    sender.send_err(err).await;
                    return;
                }
            };

        let mut resume = self.state.gateway.resume.take().unwrap_or_default();
        resume.session_id = session_id;

        log::debug!("New resume argument: {:?}", resume);

        let (sink, stream) = message_stream.split();

        log::debug!("Move to streaming state");

        ClientInner {
            state: ClientStateStreaming {
                gateway: self.state.gateway,
                sender,
                sink,
                stream,
            },
        }
        .streaming_start();
    }
}
