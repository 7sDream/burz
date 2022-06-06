use serde::{Deserialize, Serialize};

/// A util structure to hold data filed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlyData<D> {
    /// data field
    #[serde(rename = "d")]
    pub data: D,
}

/// Hello message data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    /// status code, zero for success
    pub code: i64,
    /// conversion session id, exist only when code is zero
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// A util structure to hold only sn field
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct SN {
    /// serial number
    pub sn: u64,
}

/// Reconnect message data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reconnect {
    /// status code for why we need reconnect
    pub code: i64,
    /// reason for human read
    pub err: String,
}

/// ReconnectACK message data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeACK {
    /// conversion session id
    pub session_id: String,
}
