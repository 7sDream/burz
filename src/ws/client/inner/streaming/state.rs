use std::fmt::Debug;

use futures_util::{
    future,
    stream::{SplitSink, SplitStream},
    FutureExt, Sink, Stream, StreamExt,
};
use tokio::{sync::watch, time::Instant};

use super::{ping::PingWorker, EventStreamErrorKind, EventStreamSender};
use crate::{
    api::types::GatewayURLInfo,
    ws::{
        client::inner::{
            timeout::ClientStateTimeout, ClientInner, STREAMING_STATE_PONG_TIMEOUT_MAX_COUNT,
        },
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
        log::debug!("Streaming background task start");

        let (sn_notifier, sn_watcher) = watch::channel(state.sender.resume.sn);
        let (pong_timeout_notifier, mut pong_timeout_watcher) = watch::channel(None);

        state.sender.sn_notifier = Some(sn_notifier);

        let mut pw_event_sender = state.sender.clone();
        pw_event_sender.sn_watcher = Some(sn_watcher);
        let pw = PingWorker::new(pw_event_sender, state.sink, pong_timeout_notifier);

        let pw_handler = tokio::spawn(pw.run());

        let mut pong_timeout_tick: Option<Instant> = None;
        let mut pong_timeout_count = 0;

        loop {
            let pong_timeout = if let Some(tick) = pong_timeout_tick {
                tokio::time::sleep_until(tick).boxed()
            } else {
                future::pending().boxed()
            };

            tokio::select! {
                biased;

                // pong timeout
                _ = pong_timeout => {
                    pong_timeout_count += 1;
                    log::warn!("Pong timeout, counts {}", pong_timeout_count);

                    log::trace!("Reset pong timeout tick to inf");
                    pong_timeout_tick = None;

                    if pong_timeout_count >= STREAMING_STATE_PONG_TIMEOUT_MAX_COUNT {
                        log::warn!("Reached pong time out count limit, move to timeout state");

                        log::trace!("Prepare move to timeout state");

                        drop(state.sender.sn_notifier.take());

                        log::trace!("Waiting ping worker to stop");

                        let sink = pw_handler.await.unwrap();
                        state.sender.sn_notifier = None;

                        log::debug!("Move to timeout state");

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

                // new ping message sent, update ping pong timeout clock
                watch_result = pong_timeout_watcher.changed() => {
                    if let Err(err) = watch_result {
                        log::debug!("Find ping worker stopped due to pong timeout watcher returning error: {}", err);
                        break
                    }

                    pong_timeout_tick = *pong_timeout_watcher.borrow();

                    log::trace!("Next pong timeout tick: {:?}", pong_timeout_tick);
                }

                // new message received
                result = state.stream.next() => {

                    match result.unwrap() {
                        Ok(message) => {
                            log::trace!("Received new {} message", message.type_name());

                            log::trace!("Reset pong timeout tick to inf and clean timeout count");
                            pong_timeout_tick = None;
                            pong_timeout_count = 0;

                            match message {
                                Message::Event(data) => {
                                    log::trace!("Received event sn = {}", data.sn);

                                    if state.sender.send_event(data.event).await {
                                        log::trace!("Send event to event stream success");
                                    } else {
                                        log::debug!("Send event to event stream failed, means receive side dropped, stop");
                                        // event receive side dropped, stop produce
                                        break;
                                    }

                                    // update event sn
                                    if state.sender.update_sn(data.sn) {
                                        log::trace!("Update sn to {} success", state.sender.resume.sn);
                                    } else {
                                        log::debug!("Find ping worker stopped due to update sn failed, stop");
                                        break
                                    }
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
                                Message::ResumeACK(_) => {
                                    // TODO: do we need update session id?
                                }
                                // Ignore other message
                                _ => {}
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

    pub fn streaming_start(self) {
        tokio::spawn(Self::streaming_background(self.state));
    }
}
