use snafu::prelude::*;

/// API Error
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), module(variant), context(suffix(false)))]
pub enum Error {
    /// bot token is invalid(contains invalid character that cant be send in HTTP header)
    #[snafu(display("bot token {token} is invalid"))]
    TokenInvalid {
        /// input token
        token: String,
    },

    /// create HTTP client failed
    #[snafu(display("create api client failed: {source}"))]
    ClientCreateFailed {
        /// source error
        source: reqwest::Error,
    },

    /// build api request failed
    #[snafu(display("build request failed: {source}"))]
    BuildRequestFailed {
        /// source error
        source: reqwest::Error,
    },

    /// send api request failed
    #[snafu(display("{} url {url} failed: {source}", method.as_str()))]
    RequestFailed {
        /// http method
        method: reqwest::Method,
        /// target url
        url: String,
        /// source http error
        source: reqwest::Error,
    },

    /// http response of api request is not OK(200)
    #[snafu(display("{} url {url} got http status code {status_code}", method.as_str()))]
    HTTPStatusNotOK {
        /// http method
        method: reqwest::Method,
        /// request url
        url: String,
        /// received http status code
        status_code: reqwest::StatusCode,
    },

    /// parse response body of api request as target json type failed
    #[snafu(display("parse response body {body:?} failed: {source}"))]
    ParseBodyFailed {
        /// http response body
        body: bytes::Bytes,
        /// source parse error
        source: serde_json::Error,
    },

    /// api response code is not zero
    #[snafu(display("api return error code {code}, {message}"))]
    CodeNotZero {
        /// received response code
        code: i64,
        /// received message
        message: String,
    },
}
