use tokio::sync::{mpsc, watch};

use crate::{api::types::GatewayResumeArguments, ws::Event};

use super::{EventStream, EventStreamError, EventStreamErrorKind};

#[derive(Debug)]
pub(crate) struct EventStreamSender {
    pub resume: GatewayResumeArguments,
    pub event_tx: mpsc::Sender<Result<Event, EventStreamError>>,
    pub sn_watcher: Option<watch::Receiver<u64>>,
    pub sn_notifier: Option<watch::Sender<u64>>,
}

impl Clone for EventStreamSender {
    fn clone(&self) -> Self {
        Self {
            resume: self.resume.clone(),
            event_tx: self.event_tx.clone(),
            sn_watcher: self.sn_watcher.clone(),
            sn_notifier: None,
        }
    }
}

impl EventStreamSender {
    pub fn new(resume: GatewayResumeArguments) -> (Self, EventStream) {
        let (event_tx, event_rx) = tokio::sync::mpsc::channel(32);

        (
            Self {
                resume,
                event_tx,
                sn_notifier: None,
                sn_watcher: None,
            },
            EventStream { rx: event_rx },
        )
    }
}

impl EventStreamSender {
    pub fn update_sn(&mut self, val: u64) -> bool {
        self.resume.sn = val;
        if let Some(ref notifier) = self.sn_notifier {
            notifier.send(val).is_ok()
        } else {
            true
        }
    }

    pub fn has_sn_watcher(&self) -> bool {
        self.sn_watcher.is_some()
    }

    pub async fn wait_sn_change(&mut self) -> bool {
        if let Some(ref mut watcher) = self.sn_watcher {
            if watcher.changed().await.is_ok() {
                let val = *watcher.borrow();
                self.update_sn(val)
            } else {
                false
            }
        } else {
            true
        }
    }

    pub async fn send_event(&self, event: Event) -> bool {
        self.event_tx.send(Ok(event)).await.is_ok()
    }

    pub async fn send_err(&self, err: EventStreamErrorKind) -> bool {
        self.event_tx
            .send(Err(EventStreamError {
                resume: self.resume.clone(),
                source: err,
            }))
            .await
            .is_ok()
    }
}
