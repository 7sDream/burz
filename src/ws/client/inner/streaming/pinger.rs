use std::{fmt::Debug, time::Duration};

use futures_util::{stream::SplitSink, Sink, SinkExt};
use snafu::prelude::*;
use tokio::{sync::watch, time::Instant};

use super::{error, EventStreamSender};
use crate::ws::message::{Message, MessageStreamSinkError};

#[derive(Debug)]
pub(crate) struct PingWorker<S> {
    pub sender: EventStreamSender,
    pub sink: SplitSink<S, Message>,
    pub timeout_tx: watch::Sender<Option<Instant>>,
}

impl<S> PingWorker<S>
where
    S: Sink<Message, Error = MessageStreamSinkError> + Debug,
{
    pub fn new(
        sender: EventStreamSender,
        sink: SplitSink<S, Message>,
        timeout_tx: watch::Sender<Option<Instant>>,
    ) -> Self {
        Self {
            sender,
            sink,
            timeout_tx,
        }
    }

    pub async fn run(mut self) -> SplitSink<S, Message> {
        log::debug!("Pinger start: {:?}", self);

        let mut ping_time = Instant::now();

        loop {
            let need_ping = tokio::time::sleep_until(ping_time);

            tokio::select! {
                biased;

                notifier_exist = self.sender.wait_sn_change(), if self.sender.has_sn_watcher() => {
                    if !notifier_exist {
                        log::debug!("Pinger find sn notifier closed when wait sn update");
                        log::debug!("Stop");
                        break
                    }
                    log::trace!("Pinger sn update to {}", self.sender.resume.sn);
                }

                _ = need_ping => {
                    log::trace!("Pinger send ping message");
                    if let Err(err) = self.sink.feed(self.sender.resume.ping()).await.context(error::MessageStream) {
                        log::debug!("Pinger find message stream broken when send ping message: {}", err);
                        log::debug!("Send error to event stream");
                        self.sender.send_err(err).await;
                        log::debug!("Stop");
                        break
                    }

                    ping_time = Instant::now() + Duration::from_secs(30);
                    log::trace!("Next ping time: {:?}", ping_time);

                    log::trace!("Send pong timeout tick to streaming background");
                    if let Err(err) = self.timeout_tx.send(Some(Instant::now() + Duration::from_secs(6))) {
                        log::debug!("Pinger find streaming background task stopped due to timeout rx returning error: {}", err);
                        log::debug!("Stop");
                        break
                    }
                }
            }
        }

        self.sink
    }
}
