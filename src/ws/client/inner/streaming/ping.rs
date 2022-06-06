use std::{fmt::Debug, time::Duration};

use futures_util::{stream::SplitSink, Sink, SinkExt};
use snafu::prelude::*;
use tokio::{sync::watch, time::Instant};

use super::{error, EventStreamSender};
use crate::ws::{
    client::inner::{PONG_TIMEOUT, STREAMING_STATE_PING_INTERVAL},
    message::{Message, MessageStreamSinkError},
};

#[derive(Debug)]
pub(crate) struct PingWorker<S> {
    pub sender: EventStreamSender,
    pub sink: SplitSink<S, Message>,
    pub pong_timeout_tick_notifier: watch::Sender<Option<Instant>>,
}

impl<S> PingWorker<S>
where
    S: Sink<Message, Error = MessageStreamSinkError> + Debug,
{
    pub fn new(
        sender: EventStreamSender,
        sink: SplitSink<S, Message>,
        pong_timeout_tick_notifier: watch::Sender<Option<Instant>>,
    ) -> Self {
        Self {
            sender,
            sink,
            pong_timeout_tick_notifier,
        }
    }

    pub async fn run(mut self) -> SplitSink<S, Message> {
        log::debug!("Ping worker start");

        let mut send_ping_tick = Instant::now();

        loop {
            let send_ping_clock = tokio::time::sleep_until(send_ping_tick);

            tokio::select! {
                biased;

                notifier_alive = self.sender.wait_sn_change(), if self.sender.has_sn_watcher() => {
                    if !notifier_alive {
                        log::debug!("Find sn notifier dead when wait sn update");
                        log::debug!("Stop");
                        break
                    }
                    log::trace!("Ping worker sn update to {}", self.sender.resume.sn);
                }

                _ = send_ping_clock => {
                    log::trace!("Send ping message with sn {}", self.sender.resume.sn);
                    if let Err(err) = self.sink.feed(self.sender.resume.ping()).await.context(error::MessageStream) {
                        log::debug!("Find message stream broken when send ping message: {}", err);
                        log::trace!("Send error to event stream");
                        self.sender.send_err(err).await;
                        log::debug!("Stop");
                        break
                    }

                    send_ping_tick = Instant::now() + Duration::from_secs(STREAMING_STATE_PING_INTERVAL);

                    log::trace!("Send pong timeout tick to streaming background task");
                    let pong_timeout_tick = Instant::now() + Duration::from_secs(PONG_TIMEOUT);
                    if let Err(err) = self.pong_timeout_tick_notifier.send(Some(pong_timeout_tick)) {
                        log::debug!("Find streaming background task stopped due to pong timeout tick notifier returning error: {}", err);
                        log::debug!("Stop");
                        break
                    }
                }
            }
        }

        self.sink
    }
}
