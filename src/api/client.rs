use std::borrow::Borrow;

use reqwest::{Method, StatusCode};
use snafu::prelude::*;

use super::error::variant::*;
use super::types::*;
use super::Result;

static BASE_URL: &str = "https://www.kaiheila.cn/api/v3";

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// Kaiheila HTTP API Client
#[derive(Debug, Clone)]
pub struct Client {
    client: reqwest::Client,
}

impl Client {
    fn new<S: AsRef<str> + ?Sized>(auth_type: &'static str, token: &S) -> Result<Self> {
        let token = token.as_ref();
        let auth_header_value = format!("{} {}", auth_type, token).parse().map_err(|_| {
            TokenInvalid {
                token: token.to_string(),
            }
            .build()
        })?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, auth_header_value);

        let client = reqwest::Client::builder()
            .gzip(true)
            .deflate(true)
            .user_agent(APP_USER_AGENT)
            .default_headers(headers)
            .build()
            .context(ClientCreateFailed)?;

        Ok(Self { client })
    }

    /// create a new api client using bot token
    pub fn new_from_bot_token<S: AsRef<str> + ?Sized>(token: &S) -> Result<Self> {
        Self::new("Bot", token)
    }

    /// create a new api client using oauth2 token
    pub fn new_from_oauth2_token<S: AsRef<str> + ?Sized>(token: &S) -> Result<Self> {
        Self::new("Bearer", token)
    }

    async fn request<R, P, Q, K, V>(&self, path: &P, query: Q) -> Result<R>
    where
        P: AsRef<str> + ?Sized,
        Q: IntoIterator,
        Q::Item: Borrow<(K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
        R: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", BASE_URL, path.as_ref());
        let mut req = self.client.get(&url);

        for q in query.into_iter() {
            let (k, v) = q.borrow();
            req = req.query(&[(k.as_ref(), v.as_ref())]);
        }

        let req = req.build().context(BuildRequestFailed)?;

        let resp = self
            .client
            .execute(req)
            .await
            .with_context(|_| RequestFailed {
                method: Method::GET,
                url: &url,
            })?;

        ensure!(
            resp.status() == StatusCode::OK,
            HTTPStatusNotOK {
                method: Method::GET,
                url: &url,
                status_code: resp.status()
            }
        );

        let body = resp.bytes().await.with_context(|_| RequestFailed {
            method: Method::GET,
            url: &url,
        })?;

        let result: Response<R> =
            serde_json::from_slice(&body).with_context(|_| ParseBodyFailed { body })?;

        ensure!(
            result.code == 0,
            CodeNotZero {
                code: result.code,
                message: result.message
            }
        );

        Ok(result.data)
    }

    /// Call /gateway/index, get gateway url
    pub async fn gateway_url(&self) -> Result<String> {
        let data: GatewayIndexData = self.request("/gateway/index", &[("compress", "1")]).await?;
        Ok(data.url)
    }
}
