use snafu::*;
use tokio_tungstenite as websocket;

use super::{connected::ClientStateConnected, ClientInner};
use crate::api::types::GatewayURLInfo;

/// Error when connect to websocket gateway
#[derive(Debug, Snafu)]
#[snafu(
    display("connect ws gateway {url} failed: {source}"),
    visibility(pub(crate)),
    module(error),
    context(suffix(false))
)]
pub struct ConnectGatewayError {
    /// connected url
    pub url: String,
    /// source error
    pub source: websocket::tungstenite::Error,
}

#[derive(Debug)]
pub(crate) struct ClientStateGateway {
    pub gateway: GatewayURLInfo,
}

impl ClientInner<ClientStateGateway> {
    pub async fn connect(self) -> Result<ClientInner<ClientStateConnected>, ConnectGatewayError> {
        let u = self.state.gateway.url();

        log::debug!("Connecting gateway: {}", u);

        let mut conn_result = websocket::connect_async(&u).await;
        if conn_result.is_err() {
            log::warn!("First try to connect gateway failed, start second try");
            conn_result = websocket::connect_async(&u).await
        }

        let ws = conn_result
            .map(|(client, _)| client)
            .with_context(|_| error::ConnectGateway { url: u })?;

        log::debug!("Move to connected state");

        Ok(ClientInner {
            state: ClientStateConnected {
                gateway: self.state.gateway,
                ws,
            },
        })
    }
}
