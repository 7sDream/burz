mod inner;

pub use inner::{
    ConnectGatewayError, EventStream, EventStreamError, EventStreamErrorKind, RunError,
    WaitHelloError,
};

use tokio_tungstenite as websocket;

use crate::api::types::{GatewayResumeArguments, GatewayURLInfo};
use inner::{ClientInner, ClientStateInit};

pub(crate) type WebsocketClient =
    websocket::WebSocketStream<websocket::MaybeTlsStream<tokio::net::TcpStream>>;

/// Kaiheila websocket protocol client, it will follow the official state machine at:
/// <https://developer.kaiheila.cn/doc/websocket#Gateway>
#[derive(Debug)]
pub struct Client {
    inner: ClientInner<ClientStateInit>,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Create a new client
    pub fn new() -> Self {
        Self {
            inner: ClientInner {
                state: ClientStateInit { resume: None },
            },
        }
    }

    /// Create a client and resume from last session
    pub fn resume(args: GatewayResumeArguments) -> Self {
        Self {
            inner: ClientInner {
                state: ClientStateInit { resume: Some(args) },
            },
        }
    }

    /// start running the client in given gateway, returning a stream for kaiheila event
    pub async fn run(self, gateway: GatewayURLInfo) -> Result<EventStream, RunError> {
        self.inner.run(gateway).await
    }
}
