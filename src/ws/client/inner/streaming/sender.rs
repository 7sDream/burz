use tokio::sync::{mpsc, watch};

use super::{EventBuffer, EventStream, EventStreamError, EventStreamErrorKind};
use crate::{
    api::types::GatewayResumeArguments,
    ws::{
        event::EventData,
        message::{MessageStreamSinkError, Reconnect},
        Event, Message,
    },
};

#[derive(Debug)]
struct SnRecorder {
    resume: GatewayResumeArguments,
    sn_watcher: Option<watch::Receiver<u64>>,
    sn_notifier: Option<watch::Sender<u64>>,
}

impl Clone for SnRecorder {
    fn clone(&self) -> Self {
        Self {
            resume: self.resume.clone(),
            sn_watcher: self.sn_watcher.clone(),
            sn_notifier: None,
        }
    }
}

impl SnRecorder {
    pub fn update_sn(&mut self, val: u64) -> bool {
        if self.resume.sn < val {
            self.resume.sn = val;
            if let Some(ref notifier) = self.sn_notifier {
                notifier.send(val).is_ok()
            } else {
                true
            }
        } else {
            true
        }
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
            false
        }
    }
}

#[derive(Debug)]
pub(crate) struct EventStreamSender {
    buffer: EventBuffer,
    event_tx: mpsc::Sender<Result<Box<Event>, EventStreamError>>,
    recorder: SnRecorder,
}

impl Clone for EventStreamSender {
    fn clone(&self) -> Self {
        Self {
            buffer: EventBuffer::default(),
            event_tx: self.event_tx.clone(),
            recorder: self.recorder.clone(),
        }
    }
}

impl EventStreamSender {
    pub fn new(resume: GatewayResumeArguments) -> (Self, EventStream) {
        let (event_tx, event_rx) = tokio::sync::mpsc::channel(32);

        (
            Self {
                buffer: EventBuffer::default(),
                event_tx,
                recorder: SnRecorder {
                    resume,
                    sn_watcher: None,
                    sn_notifier: None,
                },
            },
            EventStream { rx: event_rx },
        )
    }

    pub fn set_sn_notifier(&mut self, notifier: watch::Sender<u64>) {
        self.recorder.sn_notifier.replace(notifier);
    }

    pub fn set_sn_watcher(&mut self, watcher: watch::Receiver<u64>) {
        self.recorder.sn_watcher.replace(watcher);
    }

    pub fn remove_sn_notifier(&mut self) {
        self.recorder.sn_notifier.take();
    }
}

impl EventStreamSender {
    pub fn resume(&self) -> &GatewayResumeArguments {
        &self.recorder.resume
    }

    pub fn sn(&self) -> u64 {
        self.recorder.resume.sn
    }

    pub async fn wait_sn_change(&mut self) -> bool {
        self.recorder.wait_sn_change().await
    }

    pub fn ping(&self) -> Message {
        self.recorder.resume.ping()
    }

    pub async fn flush(&mut self) -> bool {
        for data in self.buffer.events_can_be_sent(self.sn()) {
            if self.event_tx.send(Ok(data.event)).await.is_ok() {
                log::trace!("Send event {} to event stream success", data.sn);
            } else {
                log::debug!(
                    "Send event {} to event stream failed, means receive side dropped, stop",
                    data.sn
                );
                // event receive side dropped, stop produce
                return false;
            }

            if !self.recorder.update_sn(data.sn) {
                return false;
            }
        }

        true
    }

    pub fn put(&mut self, event: EventData) {
        self.buffer.put(self.sn(), event);
    }

    pub async fn send_event(&mut self, event: EventData) -> bool {
        self.put(event);
        self.flush().await
    }

    pub async fn send_err(&self, err: EventStreamErrorKind) -> bool {
        self.event_tx
            .send(Err(EventStreamError {
                resume: self.recorder.resume.clone(),
                source: err,
            }))
            .await
            .is_ok()
    }

    pub async fn send_reconnect(&self, data: Reconnect) {
        log::trace!("Send reconnect error to event stream");
        self.send_err(EventStreamErrorKind::Reconnect {
            code: data.code,
            message: data.err,
        })
        .await;
    }

    pub async fn send_message_stream_broken(&self, err: MessageStreamSinkError) {
        log::trace!("Send message stream broken error to event stream");
        self.send_err(EventStreamErrorKind::MessageStream {
            source: Box::new(err),
        })
        .await;
    }
}
