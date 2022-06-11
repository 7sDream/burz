use std::{fmt::Debug, time::Duration};

use futures_util::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream, StreamExt,
};
use snafu::prelude::*;
use tokio::time::Instant;

use super::{
    connected::ClientStateConnected,
    streaming::error,
    streaming::{self, ClientStateStreaming, EventStreamSender},
    ClientInner, ClientStateInit,
};
use crate::{
    api::types::GatewayURLInfo,
    ws::{
        client::inner::{
            PONG_TIMEOUT, TIMEOUT_STATE_SEND_PING_INTERVAL_MAX,
            TIMEOUT_STATE_SEND_PING_INTERVAL_START,
        },
        message::{Message, MessageStreamSinkError},
    },
};

pub(crate) struct ClientStateTimeout<S> {
    pub gateway: Option<GatewayURLInfo>,
    pub sender: EventStreamSender,
    pub sink: SplitSink<S, Message>,
    pub stream: SplitStream<S>,
}

impl<S> ClientStateTimeout<S>
where
    S: Sink<Message, Error = MessageStreamSinkError>
        + Stream<Item = Result<Message, MessageStreamSinkError>>
        + Debug
        + Send
        + Unpin
        + 'static,
{
    pub fn into_streaming(self) -> ClientStateStreaming<S> {
        ClientStateStreaming::<S> {
            gateway: self.gateway.unwrap(),
            sender: self.sender,
            sink: Some(self.sink),
            stream: self.stream,
        }
    }

    async fn reconnect(&mut self) -> Option<ClientStateConnected> {
        let client = ClientInner {
            state: ClientStateInit {
                resume: Some(self.sender.resume().clone()),
            },
        };

        match client
            .into_gateway(self.gateway.take().unwrap())
            .connect()
            .await
            .context(error::ReConnectGatewayFailed)
        {
            Ok(c) => Some(c.state),
            Err(err) => {
                self.sender.send_err(err).await;
                None
            }
        }
    }

    async fn on_message(mut self, message: Result<Message, MessageStreamSinkError>) {
        match message {
            Ok(message) => {
                log::trace!("Received new message type: {}", message.type_name());

                match message {
                    Message::Reconnect(data) => {
                        self.sender.send_reconnect(data.data).await;
                        log::debug!("Stop");
                    }
                    _ => {
                        if let Ok(data) = message.into_event() {
                            self.sender.put(data);
                        }

                        log::info!("Recovery from timeout state");

                        let streaming = self.into_streaming();
                        let client = ClientInner { state: streaming };

                        log::debug!("Move to streaming state");
                        client.streaming_start();
                    }
                }
            }
            Err(err) => {
                log::warn!("Find message stream broken when receive message: {}", err);
                self.sender.send_message_stream_broken(err).await;
                log::debug!("Stop");
            }
        };
    }

    pub async fn waiting(mut self) {
        log::debug!("Timeout background task start");

        let pong_timeout_clock = tokio::time::sleep(Duration::from_secs(PONG_TIMEOUT));
        tokio::pin!(pong_timeout_clock);

        let mut send_ping_delay = 0;
        let mut send_ping_tick = Instant::now();

        loop {
            tokio::select! {
                biased;

                _ = &mut pong_timeout_clock => {
                    log::warn!("Pong still timeout, reconnect to gateway");

                    if let Some(connected) = self.reconnect().await {
                        log::debug!("Reconnect success");
                        let client = ClientInner { state: connected};
                        client.re_wait_hello(self.sender).await;
                    }

                    return;
                }

                _ = tokio::time::sleep_until(send_ping_tick) => {
                    log::trace!("Send ping with sn {}", self.sender.sn());

                    if let Err(err) = self
                    .sink
                    .feed(self.sender.ping())
                    .await
                    .context(streaming::error::MessageStream)
                    {
                        log::debug!("Find message stream broken when send ping message: {}", err);
                        log::trace!("Send error to event stream");
                        self.sender.send_err(err).await;
                        log::trace!("Stop");
                        return;
                    }

                    send_ping_delay *= 2;
                    send_ping_delay = send_ping_delay.clamp(TIMEOUT_STATE_SEND_PING_INTERVAL_START, TIMEOUT_STATE_SEND_PING_INTERVAL_MAX);

                    log::trace!("Next ping in {} seconds", send_ping_delay);

                    send_ping_tick = Instant::now() + Duration::from_secs(send_ping_delay);
                }

                result = self.stream.next() => {
                    self.on_message(result.unwrap()).await;
                    return;
                }
            }
        }
    }
}

impl<S> ClientInner<ClientStateTimeout<S>>
where
    S: Sink<Message, Error = MessageStreamSinkError>
        + Stream<Item = Result<Message, MessageStreamSinkError>>
        + Debug
        + Send
        + Unpin
        + 'static,
{
    pub fn timeout_start(self) {
        tokio::spawn(self.state.waiting());
    }
}
