//! Kaiheila websocket events in [Event](super::message::Message::Event) message type.

mod types;

pub use types::*;

use serde::{Deserialize, Serialize};

/// Event data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventData {
    /// serial number
    pub sn: u64,

    /// event body
    #[serde(rename = "d")]
    pub event: Box<Event>,
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
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// 消息通道类型, GROUP 为组播消息, PERSON 为单播消息, BROADCAST 为广播消息
    pub channel_type: String,
    /// 1:文字消息, 2:图片消息，3:视频消息，4:文件消息， 8:音频消息，9:KMarkdown，10:card 消息，255:系统消息, 其它的暂未开放
    pub r#type: i64,
    /// 发送目的, 频道消息类时, 代表的是频道 channel_id，如果 channel_type 为 GROUP 组播且 type 为 255 系统消息时，则代表服务器 guild_id
    pub target_id: String,
    /// 发送者 id, 1 代表系统
    pub author_id: String,
    /// 消息内容, 文件，图片，视频时，content 为 url
    pub content: String,
    /// 消息的 id
    pub msg_id: String,
    /// 消息发送时间的毫秒时间戳
    pub msg_timestamp: i64,
    /// 随机串，与用户消息发送 api 中传的 nonce 保持一致
    pub nonce: String,
    /// 不同的消息类型，结构不一致
    pub extra: EventExtra,
}

/// Extra info for an event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventExtra {
    /// type = 1, text message
    TextMessage(TextMessageExtra),
}

impl Default for EventExtra {
    fn default() -> Self {
        Self::TextMessage(TextMessageExtra::default())
    }
}

/// Extra info for text message
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextMessageExtra {
    /// const 1
    pub r#type: i64,
    /// 服务器 id
    pub guild_id: String,
    /// 频道名
    pub channel_name: String,
    /// 提及到的用户 id 的列表
    pub mention: Vec<String>,
    /// 是否 mention 所有用户
    pub mention_all: bool,
    ///  mention 用户角色的数组
    pub mention_roles: Vec<u64>,
    /// 是否 mention 在线用户
    pub mention_here: bool,
    /// 发消息用户信息
    pub author: User,
    /// 引用消息
    pub quote: Option<Quote>,
}
