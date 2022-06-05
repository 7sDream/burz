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

#[derive(Debug)]
pub(crate) struct ClientInner<S> {
    pub state: S,
}
