//! Kaiheila websocket events in [Event](super::message::Message::Event) message type.

use serde::{Deserialize, Serialize};

/// Event data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventData {
    /// serial number
    pub sn: u64,

    /// event body
    #[serde(rename = "d")]
    pub event: Event,
}

impl PartialOrd for EventData {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.sn.partial_cmp(&other.sn)
    }
}

impl Ord for EventData {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sn.cmp(&other.sn)
    }
}

/// Event type
pub type Event = serde_json::Value;

// Event
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct Event {}
