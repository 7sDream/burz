use std::{fmt::Debug, time::Duration};

use futures_util::{
    future,
    stream::{SplitSink, SplitStream},
    FutureExt, Sink, SinkExt, Stream, StreamExt,
};
use snafu::prelude::*;
use tokio::time::Instant;

use super::{
    gateway::ClientStateGateway,
    streaming::error,
    streaming::{self, ClientStateStreaming, EventStreamSender},
    ClientInner, EventStreamErrorKind,
};
use crate::{
    api::types::GatewayURLInfo,
    ws::message::{Message, MessageStreamSinkError},
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

async fn to_gateway<S>(mut state: ClientStateTimeout<S>)
where
    S: Sink<Message, Error = MessageStreamSinkError>
        + Stream<Item = Result<Message, MessageStreamSinkError>>
        + Send
        + Unpin
        + 'static,
{
    state.gateway.resume.replace(state.sender.resume.clone());
    let client = ClientInner {
        state: ClientStateGateway {
            gateway: state.gateway,
        },
    };

    let client = match client
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
        let mut timeout = None;

        let mut time_send_ping = Instant::now() + Duration::from_secs(2);
        let mut ping_send_count = 0;

        loop {
            let timeout_clock = if let Some(tick) = timeout {
                tokio::time::sleep_until(tick).boxed()
            } else {
                future::pending().boxed()
            };

            tokio::select! {
                biased;

                _ = timeout_clock => {
                    to_gateway(state).await;
                    break;
                }

                _ = tokio::time::sleep_until(time_send_ping), if ping_send_count < 2 => {
                    ping_send_count += 1;
                    if let Err(err) = state
                    .sink
                    .feed(state.sender.resume.ping())
                    .await
                    .context(streaming::error::MessageStream)
                    {
                        state.sender.send_err(err).await;
                        return;
                    } else {
                        timeout = Some(Instant::now() + Duration::from_secs(6));
                        time_send_ping = Instant::now() + Duration::from_secs(4);
                    }
                }

                result = state.stream.next() => {
                    match result.unwrap() {
                        Ok(message) => match message {
                            Message::Event(data) => {
                                // update event sn
                                state.sender.update_sn(data.sn);
                                if !state.sender.send_event(data.event).await {
                                    // event receive side dropped, stop produce
                                    break;
                                }

                                to_streaming(state);
                                break;
                            }
                            Message::Reconnect(data) => {
                                // reconnect received, stop
                                state.sender.send_err(EventStreamErrorKind::Reconnect {
                                    code: data.data.code,
                                    message: data.data.err,
                                }).await;
                                break;
                            }
                            _ => {
                                to_streaming(state);
                                break;
                            }
                        }
                        Err(err) => {
                            // message stream broken, stop
                            let err = EventStreamErrorKind::MessageStream {
                                source: Box::new(err),
                            };
                            state.sender.send_err(err).await;
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
