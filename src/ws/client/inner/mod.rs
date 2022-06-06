mod connected;
mod gateway;
mod init;
mod streaming;
mod timeout;

pub(super) use init::ClientStateInit;

pub use connected::WaitHelloError;
pub use gateway::ConnectGatewayError;
pub use init::RunError;
pub use streaming::{EventStream, EventStreamError, EventStreamErrorKind};

pub(crate) const PONG_TIMEOUT: u64 = 6;

pub(crate) const STREAMING_STATE_PING_INTERVAL: u64 = 30;
pub(crate) const STREAMING_STATE_PONG_TIMEOUT_MAX_COUNT: usize = 2;

pub(crate) const TIMEOUT_STATE_SEND_PING_INTERVAL_START: u64 = 2;
pub(crate) const TIMEOUT_STATE_SEND_PING_INTERVAL_MAX: u64 = PONG_TIMEOUT;

#[derive(Debug)]
pub(crate) struct ClientInner<S> {
    pub state: S,
}
