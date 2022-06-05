//! # Burz
//!
//! A Kaiheila bot framework.

#![deny(warnings)]
#![deny(clippy::all)]
#![deny(missing_debug_implementations, missing_docs)]
#![forbid(unsafe_code)]

pub mod api;
pub mod ws;

mod error;
pub use error::Error;

use futures_util::StreamExt;
use snafu::prelude::*;

/// framework result type
pub type Result<T> = std::result::Result<T, Error>;

/// Burz instance
#[derive(Debug)]
pub struct Bot {
    api_client: api::Client,
}

impl Bot {
    /// Create new framework instance using bot token
    pub fn new<S: AsRef<str> + ?Sized>(token: &S) -> Result<Self> {
        let api_client = api::Client::new_from_bot_token(&token).context(error::CallAPIFailed)?;

        log::info!("Crate api and websocket client success");

        Ok(Self { api_client })
    }

    /// Run
    pub async fn run(self) -> Result<()> {
        let mut resume = None;

        loop {
            log::info!("Getting gateway url...");

            let gateway = self
                .api_client
                .gateway_url()
                .await
                .context(error::CallAPIFailed)?;

            log::debug!("Got gateway url: {}", gateway);

            let gateway_info = gateway
                .parse()
                .with_context(|_| error::InvalidGatewayURL { url: &gateway })?;

            let ws_client = if let Some(r) = resume.take() {
                log::debug!("Resume conversion using argument: {:?}", r);
                ws::Client::resume(r)
            } else {
                ws::Client::new()
            };

            log::debug!("Connecting gateway ...");

            let mut stream = ws_client
                .run(gateway_info)
                .await
                .context(error::RunWebsocketClientFailed)?;

            log::info!("Gateway connected, start receiving events");

            loop {
                let item = stream.next().await.unwrap();
                match item {
                    Ok(event) => {
                        log::info!("Received event: {:?}", event)
                    }
                    Err(err) => {
                        log::warn!("EventStream broken, reason: {}", err.source);
                        log::debug!("Resume argument: {:?}", err.resume);

                        resume.replace(err.resume);

                        log::info!("Bot Restart")
                    }
                }
            }
        }
    }
}
