use std::time::Duration;

use futures_util::StreamExt;
use snafu::prelude::*;

use crate::{
    api::{self, types::GatewayURLInfo},
    error, ws, Result,
};

const RE_FETCH_GATEWAY_INTERVAL_MAX: u64 = 60;

/// Burz instance
#[derive(Debug)]
pub struct Bot {
    #[allow(dead_code)]
    api_client: api::Client,
}

impl Bot {
    /// Create new framework instance using bot token
    pub fn new<S: AsRef<str> + ?Sized>(token: &S) -> Result<Self> {
        let api_client = api::Client::new_from_bot_token(&token).context(error::CallAPIFailed)?;

        log::info!("Crate api and websocket client success");

        Ok(Self { api_client })
    }

    // async fn fetch_new_gateway(&self) -> Result<GatewayURLInfo> {
    //     let gateway_str = self
    //         .api_client
    //         .gateway_url()
    //         .await
    //         .context(error::CallAPIFailed)?;

    //     gateway_str
    //         .parse()
    //         .with_context(|_| error::InvalidGatewayURL { url: &gateway_url })
    // }

    async fn fetch_new_gateway(&self) -> Result<GatewayURLInfo> {
        Ok("ws://127.0.0.1:7777/gateway?token=x&compress=0"
            .parse()
            .unwrap())
    }

    /// Run
    pub async fn run(self) -> Result<()> {
        let mut resume = None;
        let mut refetch_delay = 1;

        loop {
            log::info!("Getting gateway url ...");

            let gateway_info = self.fetch_new_gateway().await?;

            log::debug!("Got gateway url: {}", gateway_info.url());

            let ws_client = if let Some(r) = resume.take() {
                log::debug!("Resume conversion using argument: {:?}", r);
                ws::Client::resume(r)
            } else {
                ws::Client::new()
            };

            let mut stream = match ws_client.run(gateway_info).await {
                Ok(stream) => stream,
                Err(err) => {
                    log::warn!("Can't establish event stream with fetched url: {}", err);
                    log::warn!(
                        "Retry fetch new gateway url after {} seconds ...",
                        refetch_delay
                    );

                    tokio::time::sleep(Duration::from_secs(refetch_delay)).await;
                    refetch_delay *= 2;
                    refetch_delay = refetch_delay.clamp(1, RE_FETCH_GATEWAY_INTERVAL_MAX);

                    continue;
                }
            };

            refetch_delay = 1;

            log::info!("Event stream established, start receiving events");

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

                        log::info!("Bot Restart");

                        break;
                    }
                }
            }
        }
    }
}
