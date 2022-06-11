use std::fmt::Debug;

use futures_util::{
    future,
    stream::{SplitSink, SplitStream},
    FutureExt, Sink, Stream, StreamExt,
};
use tokio::{sync::watch, task::JoinHandle, time::Instant};

use super::{ping::PingWorker, EventStreamSender};
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
    pub sink: Option<SplitSink<S, Message>>,
    pub stream: SplitStream<S>,
}

impl<S> ClientStateStreaming<S>
where
    S: Stream<Item = Result<Message, MessageStreamSinkError>>
        + Sink<Message, Error = MessageStreamSinkError>
        + Debug
        + Send
        + Unpin
        + 'static,
{
    fn create_ping_worker(
        &mut self,
    ) -> (
        JoinHandle<SplitSink<S, Message>>,
        watch::Receiver<Option<Instant>>,
    ) {
        let (sn_notifier, sn_watcher) = watch::channel(self.sender.sn());
        let (pong_timeout_notifier, pong_timeout_watcher) = watch::channel(None);

        self.sender.set_sn_notifier(sn_notifier);

        let mut pw_event_sender = self.sender.clone();
        pw_event_sender.set_sn_watcher(sn_watcher);

        let pw = PingWorker::new(
            pw_event_sender,
            self.sink.take().unwrap(),
            pong_timeout_notifier,
        );

        let pw_handler = tokio::spawn(pw.run());

        (pw_handler, pong_timeout_watcher)
    }

    async fn into_timeout(
        mut self,
        pw_handler: JoinHandle<SplitSink<S, Message>>,
    ) -> ClientStateTimeout<S> {
        self.sender.remove_sn_notifier();

        log::trace!("Waiting ping worker to stop");
        let sink = pw_handler.await.unwrap();

        ClientStateTimeout::<S> {
            gateway: Some(self.gateway),
            sender: self.sender,
            sink,
            stream: self.stream,
        }
    }

    async fn on_message(&mut self, data: Option<Result<Message, MessageStreamSinkError>>) -> bool {
        match data.unwrap() {
            Ok(message) => {
                log::trace!("Received new message type: {}", message.type_name());

                match message {
                    Message::Event(data) => {
                        log::trace!("Received event sn = {}", data.sn);
                        self.sender.send_event(data).await
                    }
                    Message::Reconnect(data) => {
                        self.sender.send_reconnect(data.data).await;
                        log::debug!("Stop");
                        false
                    }
                    Message::ResumeACK(_) => {
                        // TODO: do we need update session id?
                        true
                    }
                    // Ignore other message
                    _ => true,
                }
            }
            Err(err) => {
                log::warn!("Find message stream broken when receive message: {}", err);
                self.sender.send_message_stream_broken(err).await;
                log::debug!("Stop");
                false
            }
        }
    }

    pub(crate) async fn streaming(mut self) {
        log::debug!("Streaming background task start");

        let (pw_handler, mut pong_timeout_watcher) = self.create_ping_worker();

        // clean events buffer, because timeout state may received new events
        if !self.sender.flush().await {
            return;
        }

        let mut pong_timeout_tick: Option<Instant> = None;
        let mut pong_timeout_count = 0;

        loop {
            let pong_timeout_clock = if let Some(tick) = pong_timeout_tick {
                tokio::time::sleep_until(tick).boxed()
            } else {
                future::pending().boxed()
            };

            tokio::select! {
                biased;

                // pong timeout
                _ = pong_timeout_clock => {
                    pong_timeout_count += 1;
                    log::warn!("Pong timeout, counts {}", pong_timeout_count);

                    log::trace!("Reset pong timeout tick to inf");
                    pong_timeout_tick = None;

                    if pong_timeout_count >= STREAMING_STATE_PONG_TIMEOUT_MAX_COUNT {
                        log::warn!("Reached pong time out count limit, move to timeout state");

                        let client = ClientInner { state: self.into_timeout(pw_handler).await };

                        log::debug!("Move to timeout state");

                        client.timeout_start();
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
                result = self.stream.next() => {
                    log::trace!("New Message received, reset pong timeout tick to inf and clean timeout count");
                    pong_timeout_tick = None;
                    pong_timeout_count = 0;

                    if !self.on_message(result).await {
                        break;
                    }
                }
            }
        }
    }
}
