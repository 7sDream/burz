use std::fmt::Debug;

use futures_util::{
    future,
    stream::{SplitSink, SplitStream},
    FutureExt, Sink, Stream, StreamExt,
};
use tokio::{sync::watch, time::Instant};

use super::{pinger::PingWorker, EventStreamErrorKind, EventStreamSender};
use crate::{
    api::types::GatewayURLInfo,
    ws::{
        client::inner::{timeout::ClientStateTimeout, ClientInner},
        message::{Message, MessageStreamSinkError},
    },
};

#[derive(Debug)]
pub(crate) struct ClientStateStreaming<S> {
    pub gateway: GatewayURLInfo,
    pub sender: EventStreamSender,
    pub sink: SplitSink<S, Message>,
    pub stream: SplitStream<S>,
}

impl<S> ClientInner<ClientStateStreaming<S>>
where
    S: Stream<Item = Result<Message, MessageStreamSinkError>>
        + Sink<Message, Error = MessageStreamSinkError>
        + Debug
        + Send
        + Unpin
        + 'static,
{
    async fn streaming_background(mut state: ClientStateStreaming<S>) {
        let (sn_notifier, sn_watcher) = watch::channel(state.sender.resume.sn);
        let (timeout_tx, mut timeout_rx) = watch::channel(None);

        state.sender.sn_notifier = Some(sn_notifier);

        log::debug!("Streaming background task start {:?}", state);

        let mut pinger_event_sender = state.sender.clone();
        pinger_event_sender.sn_watcher = Some(sn_watcher);
        let pinger = PingWorker::new(pinger_event_sender, state.sink, timeout_tx);

        let pinger_handler = tokio::spawn(pinger.run());

        let mut ping_timeout: Option<Instant> = None;
        let mut ping_timeout_count = 0u8;

        loop {
            let ping_timeout_clock = if let Some(tick) = ping_timeout {
                tokio::time::sleep_until(tick).boxed()
            } else {
                future::pending().boxed()
            };

            tokio::select! {
                biased;

                // ping timeout
                _ = ping_timeout_clock => {
                    ping_timeout_count += 1;
                    ping_timeout = None;
                    if ping_timeout_count >= 2 { // timeout twice, goto timeout state
                        drop(state.sender.sn_notifier.take());
                        let sink = pinger_handler.await.unwrap();
                        state.sender.sn_notifier = None;

                        let timeout_inner = ClientInner{
                            state: ClientStateTimeout::<S> {
                                gateway: state.gateway,
                                sender: state.sender,
                                sink,
                                stream: state.stream,
                            }
                        };
                        timeout_inner.timeout_start();
                        break;
                    }
                }

                // new ping message sent, update ping timeout clock
                next_ping_timeout = timeout_rx.changed() => {
                    if let Err(err) = next_ping_timeout {
                        log::debug!("Streaming background task find pinger stopped due to timeout rx returning error: {}", err);
                        break
                    }

                    ping_timeout = *timeout_rx.borrow();

                    log::trace!("Next pong timeout tick: {:?}", ping_timeout);
                }

                // new message received
                result = state.stream.next() => {

                    match result.unwrap() {
                        Ok(message) => {
                            log::trace!("Received new {} message", message.type_name());

                            // reset ping timeout
                            ping_timeout = None;
                            ping_timeout_count = 0;

                            log::trace!("Reset pong timeout to inf");

                            match message {
                                Message::Event(data) => {
                                    if state.sender.send_event(data.event).await {
                                        log::trace!("Send event to event stream success");
                                    } else {
                                        log::trace!("Send event to receive stream failed, means receive side dropped, exit");
                                        // event receive side dropped, stop produce
                                        break;
                                    }

                                    // update event sn
                                    if state.sender.update_sn(data.sn) {
                                        log::trace!("Update sn to {} success", data.sn);
                                    } else {
                                        log::debug!("Streaming background task find pinger stopped due to update sn notify failed");
                                        // pinger stopped, only when write ping failed, so we stop too
                                        break
                                    }
                                }
                                Message::Reconnect(data) => {
                                    // reconnect received, stop
                                    state.sender.send_err(EventStreamErrorKind::Reconnect {
                                        code: data.data.code,
                                        message: data.data.err,
                                    }).await;
                                    break;
                                }
                                Message::ResumeACK(_) => {
                                    // TODO: do we need update session id?
                                }
                                // Ignore other message
                                _ => {}
                            }
                        }
                        Err(err) => {
                            log::debug!("Streaming background task find message stream broken when receive message: {}", err);
                            log::debug!("Send error to event stream");
                            let err = EventStreamErrorKind::MessageStream {
                                source: Box::new(err),
                            };
                            state.sender.send_err(err).await;
                            log::debug!("Stop");
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn streaming_start(self) {
        tokio::spawn(Self::streaming_background(self.state));
    }
}
