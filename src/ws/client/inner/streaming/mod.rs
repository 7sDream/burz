mod buffer;
mod ping;
mod sender;
mod state;
mod stream;

pub(crate) use buffer::EventBuffer;
pub(crate) use sender::EventStreamSender;
pub(crate) use state::ClientStateStreaming;
pub(crate) use stream::error;

pub use stream::{EventStream, EventStreamError, EventStreamErrorKind};

// =====

use std::fmt::Debug;

use futures_util::{Sink, Stream};

use super::ClientInner;
use crate::ws::{message::MessageStreamSinkError, Message};

impl<S> ClientInner<ClientStateStreaming<S>>
where
    S: Stream<Item = Result<Message, MessageStreamSinkError>>
        + Sink<Message, Error = MessageStreamSinkError>
        + Debug
        + Send
        + Unpin
        + 'static,
{
    pub fn streaming_start(self) {
        tokio::spawn(self.state.streaming());
    }
}
