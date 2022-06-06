//! Kaiheila websocket events in [Event](super::message::Message::Event) message type.

use serde::{Deserialize, Serialize};

/// Event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    /// serial number
    pub sn: u64,

    /// event body
    #[serde(rename = "d")]
    pub event: Event,
}

/// Event type
pub type Event = serde_json::Value;

// Event
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct Event {}
