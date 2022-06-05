use snafu::prelude::*;

use super::{
    gateway::ClientStateGateway, ClientInner, ConnectGatewayError, EventStream, WaitHelloError,
};
use crate::api::types::{GatewayResumeArguments, GatewayURLInfo};

/// Error when run websocket client
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), module(error), context(suffix(false)))]
pub enum RunError {
    /// connect to websocket gateway failed
    #[snafu(display("connect ws gateway failed: {source}"))]
    ConnectGatewayFailed {
        /// source error
        source: ConnectGatewayError,
    },

    /// wait first server hello message failed
    #[snafu(display("wait server hello message failed: {source}"))]
    WaitHelloFailed {
        /// source error
        source: WaitHelloError,
    },
}

#[derive(Debug)]
pub(crate) struct ClientStateInit {
    pub resume: Option<GatewayResumeArguments>,
}

impl ClientInner<ClientStateInit> {
    pub async fn run(self, gateway: GatewayURLInfo) -> Result<EventStream, RunError> {
        self.into_gateway(gateway)
            .connect()
            .await
            .context(error::ConnectGatewayFailed)?
            .wait_hello()
            .await
            .context(error::WaitHelloFailed)
    }

    pub(crate) fn into_gateway(
        mut self,
        mut gateway: GatewayURLInfo,
    ) -> ClientInner<ClientStateGateway> {
        std::mem::swap(&mut gateway.resume, &mut self.state.resume);

        ClientInner {
            state: ClientStateGateway { gateway },
        }
    }
}
