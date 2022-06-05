mod pinger;
mod sender;
mod state;
mod stream;

pub(crate) use sender::EventStreamSender;
pub(crate) use state::ClientStateStreaming;
pub(crate) use stream::error;

pub use stream::{EventStream, EventStreamError, EventStreamErrorKind};
