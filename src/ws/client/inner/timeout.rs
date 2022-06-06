use std::{fmt::Debug, time::Duration};

use futures_util::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream, StreamExt,
};
use snafu::prelude::*;
use tokio::time::Instant;

use super::{
    streaming::error,
    streaming::{self, ClientStateStreaming, EventStreamSender},
    ClientInner, ClientStateInit, EventStreamErrorKind,
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
    pub gateway: GatewayURLInfo,
    pub sender: EventStreamSender,
    pub sink: SplitSink<S, Message>,
    pub stream: SplitStream<S>,
}

fn to_streaming<S>(state: ClientStateTimeout<S>)
where
    S: Sink<Message, Error = MessageStreamSinkError>
        + Stream<Item = Result<Message, MessageStreamSinkError>>
        + Debug
        + Send
        + Unpin
        + 'static,
{
    let client = ClientInner {
        state: ClientStateStreaming::<S> {
            gateway: state.gateway,
            sender: state.sender,
            sink: state.sink,
            stream: state.stream,
        },
    };

    client.streaming_start();
}

async fn to_init_with_gateway<S>(state: ClientStateTimeout<S>)
where
    S: Sink<Message, Error = MessageStreamSinkError>
        + Stream<Item = Result<Message, MessageStreamSinkError>>
        + Send
        + Unpin
        + 'static,
{
    let client = ClientInner {
        state: ClientStateInit {
            resume: Some(state.sender.resume.clone()),
        },
    };

    let client = match client
        .into_gateway(state.gateway)
        .connect()
        .await
        .context(error::ReConnectGatewayFailed)
    {
        Ok(c) => c,
        Err(err) => {
            state.sender.send_err(err).await;
            return;
        }
    };

    client.re_wait_hello(state.sender).await
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
    async fn timeout_background(mut state: ClientStateTimeout<S>) {
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

                    log::debug!("Move to init(with gateway) state");
                    to_init_with_gateway(state).await;
                    break;
                }

                _ = tokio::time::sleep_until(send_ping_tick) => {
                    log::trace!("Send ping with sn {}", state.sender.resume.sn);

                    if let Err(err) = state
                    .sink
                    .feed(state.sender.resume.ping())
                    .await
                    .context(streaming::error::MessageStream)
                    {
                        log::debug!("Find message stream broken when send ping message: {}", err);
                        log::trace!("Send error to event stream");
                        state.sender.send_err(err).await;
                        log::trace!("Stop");
                        return;
                    }

                    send_ping_delay *= 2;
                    send_ping_delay = send_ping_delay.clamp(TIMEOUT_STATE_SEND_PING_INTERVAL_START, TIMEOUT_STATE_SEND_PING_INTERVAL_MAX);

                    log::trace!("Next ping in {} seconds", send_ping_delay);

                    send_ping_tick = Instant::now() + Duration::from_secs(send_ping_delay);
                }

                result = state.stream.next() => {
                    match result.unwrap() {
                        Ok(message) => {
                            log::trace!("Received new {} message", message.type_name());

                            match message {
                                Message::Event(data) => {
                                    log::trace!("Received event sn = {}", data.sn);

                                    if state.sender.send_event(data.event).await {
                                        log::trace!("Send event to event stream success");
                                    } else {
                                        log::debug!("Send event to event stream failed, means receive side dropped, stop");
                                        break;
                                    }

                                    state.sender.update_sn(data.sn);
                                    log::trace!("Update sn to {}", state.sender.resume.sn);

                                    log::info!("Recovery from timeout state");
                                    log::debug!("Move to streaming state");
                                    to_streaming(state);
                                    break;
                                }
                                Message::Reconnect(data) => {
                                    log::trace!("Send error to event stream");
                                    state.sender.send_err(EventStreamErrorKind::Reconnect {
                                        code: data.data.code,
                                        message: data.data.err,
                                    }).await;

                                    log::debug!("Stop");
                                    break;
                                }
                                _ => {
                                    log::info!("Recovery from timeout state");
                                    log::debug!("Move to streaming state");
                                    to_streaming(state);
                                    break;
                                }
                            }
                        }
                        Err(err) => {
                            log::warn!("Find message stream broken when receive message: {}", err);

                            let err = EventStreamErrorKind::MessageStream {
                                source: Box::new(err),
                            };
                            log::trace!("Send error to event stream");
                            state.sender.send_err(err).await;

                            log::debug!("Stop");
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn timeout_start(self) {
        tokio::spawn(Self::timeout_background(self.state));
    }
}
