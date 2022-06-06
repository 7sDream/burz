//! Kaiheila websocket message types.

mod stream;
mod types;

pub use stream::{MessageStreamSink, MessageStreamSinkError};
pub use types::{Hello, OnlyData, Reconnect, ResumeACK, SN};

use bytes::Bytes;
use enum_as_inner::EnumAsInner;
use miniz_oxide::inflate::{self, TINFLStatus};
use serde::{Deserialize, Serialize};
use snafu::prelude::*;

use super::event::EventData;

/// Error when parse binary data as message
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)), module(error), context(suffix(false)))]
pub enum ParseMessageError {
    /// Decompress data failed
    #[snafu(display("decompress message failed: {status:?}"))]
    DecompressFailed {
        /// data for decode
        data: Bytes,
        /// decompress error status code
        status: TINFLStatus,
    },

    /// data is invalid json
    #[snafu(display("parse json failed: {source:?}"))]
    ParseJSONFailed {
        /// data for decode
        data: Bytes,
        /// source error
        source: serde_json::Error,
    },

    /// data json is not an object
    #[snafu(display("parsed message is not object: {json}"))]
    MessageNotObject {
        /// json string
        json: String,
    },

    /// data json has no type(s) field
    #[snafu(display("message has no type(s) field: {json}"))]
    NoMessageType {
        /// json string
        json: String,
    },

    /// data json type filed is not number type
    #[snafu(display("message has non-number s field: {json}"))]
    MessageTypeNotNumber {
        /// json string
        json: String,
    },

    /// data json has an unknown type
    #[snafu(display("message has unknown event type {t}"))]
    UnknownMessageType {
        /// type number
        t: i64,
    },

    /// data json is not valid typed message
    #[snafu(display("parse to {type_name} message failed: {source}"))]
    ParseJSONToTypedMessageFailed {
        /// type name
        type_name: String,
        /// source error
        source: serde_json::Error,
    },
}

static MESSAGE_INTERNAL_TYPE_TAG: &str = "__internal_type_tag__";

/// Kaiheila websocket protocol message type
#[derive(Debug, Clone, Serialize, Deserialize, EnumAsInner)]
// serde does not support number tag for now, see: https://github.com/serde-rs/serde/issues/745
#[serde(tag = "__internal_type_tag__")]
pub enum Message {
    /// Event, server -> client
    Event(EventData),
    /// Hello, server -> client
    Hello(OnlyData<Hello>),
    /// Ping, client -> server
    Ping(SN),
    /// Pong, server -> client
    Pong,
    /// Resume, client -> server
    Resume(SN),
    /// Reconnect, server -> client
    Reconnect(OnlyData<Reconnect>),
    /// ResumeACK, server -> client
    ResumeACK(OnlyData<ResumeACK>),
}

impl Message {
    /// Decode data to a message
    pub fn decode(mut buff: Bytes, compressed: bool) -> Result<Self, ParseMessageError> {
        if compressed {
            buff = inflate::decompress_to_vec_zlib(&buff)
                .map_err(|e| ParseMessageError::DecompressFailed {
                    data: buff.clone(),
                    status: e,
                })?
                .into();
        }

        let mut value: serde_json::Value =
            serde_json::from_slice(&buff).context(error::ParseJSONFailed { data: buff.clone() })?;

        let obj = value
            .as_object_mut()
            .with_context(|| error::MessageNotObject {
                json: String::from_utf8_lossy(&buff),
            })?;

        let s = obj
            .get("s")
            .with_context(|| error::NoMessageType {
                json: String::from_utf8_lossy(&buff),
            })?
            .as_i64()
            .with_context(|| error::MessageTypeNotNumber {
                json: String::from_utf8_lossy(&buff),
            })?;

        let type_name = Self::type_number_to_type_name(s)
            .with_context(|| error::UnknownMessageType { t: s })?;

        obj.insert(
            MESSAGE_INTERNAL_TYPE_TAG.to_string(),
            serde_json::Value::String(type_name.to_string()),
        );

        serde_json::from_value(value).with_context(|_| error::ParseJSONToTypedMessageFailed {
            type_name: type_name.to_string(),
        })
    }

    /// encode data to binary message(without compress)
    pub fn encode(&self) -> Vec<u8> {
        let mut value = serde_json::to_value(&self).unwrap();
        let obj = value.as_object_mut().unwrap();
        obj.remove(MESSAGE_INTERNAL_TYPE_TAG);
        obj.insert(
            "s".to_string(),
            serde_json::Value::Number(self.type_number().into()),
        );
        serde_json::to_vec(&value).unwrap()
    }

    fn type_number_to_type_name(s: i64) -> Option<&'static str> {
        match s {
            0 => Some("Event"),
            1 => Some("Hello"),
            2 => Some("Ping"),
            3 => Some("Pong"),
            4 => Some("Resume"),
            5 => Some("Reconnect"),
            6 => Some("ResumeACK"),
            _ => None,
        }
    }

    /// get type number
    pub fn type_number(&self) -> i64 {
        match self {
            Self::Event(_) => 0,
            Self::Hello(_) => 1,
            Self::Ping(_) => 2,
            Self::Pong => 3,
            Self::Resume(_) => 4,
            Self::Reconnect(_) => 5,
            Self::ResumeACK(_) => 6,
        }
    }

    /// get type name
    pub fn type_name(&self) -> &'static str {
        Self::type_number_to_type_name(self.type_number()).unwrap()
    }
}

#[cfg(test)]
mod test {
    mod decode {
        use super::super::*;
        use serde_json::json;

        #[test]
        fn test_message_decode_hello() {
            let data = serde_json::to_vec(&json!({
                "s": 1,
                "d": {
                    "code": 0,
                    "session_id": "some-session-id",
                },
            }))
            .unwrap()
            .into();

            let msg = Message::decode(data, false).unwrap();

            if let Message::Hello(hello) = msg {
                assert_eq!(hello.data.code, 0);
                assert_eq!(hello.data.session_id.unwrap(), "some-session-id");
            } else {
                panic!("decoded message is not hello")
            }
        }

        #[test]
        fn test_message_decode_ping() {
            let data = serde_json::to_vec(&json!({
                "s": 2,
                "sn": 6,
            }))
            .unwrap()
            .into();

            let msg = Message::decode(data, false).unwrap();

            if let Message::Ping(sn) = msg {
                assert_eq!(sn.sn, 6);
            } else {
                panic!("decoded message is not ping")
            }
        }

        #[test]
        fn test_message_parse_pong() {
            let data = serde_json::to_vec(&json!({
                "s": 3,
            }))
            .unwrap()
            .into();

            let msg = Message::decode(data, false).unwrap();

            assert!(matches!(msg, Message::Pong));
        }

        #[test]
        fn test_message_decode_resume() {
            let data = serde_json::to_vec(&json!({
                "s": 4,
                "sn": 100,
            }))
            .unwrap()
            .into();

            let msg = Message::decode(data, false).unwrap();

            if let Message::Resume(resume) = msg {
                assert_eq!(resume.sn, 100);
            } else {
                panic!("decoded message is not resume")
            }
        }

        #[test]
        fn test_message_decode_reconnect() {
            let data = serde_json::to_vec(&json!({
                "s": 5,
                "d": {
                    "code": 41008,
                    "err": "Missing params",
                },
            }))
            .unwrap()
            .into();

            let msg = Message::decode(data, false).unwrap();

            if let Message::Reconnect(reconnect) = msg {
                assert_eq!(reconnect.data.code, 41008);
                assert_eq!(reconnect.data.err, "Missing params");
            } else {
                panic!("decoded message is not reconnect")
            }
        }

        #[test]
        fn test_message_decode_resume_ack() {
            let data = serde_json::to_vec(&json!({
                "s": 6,
                "d": {
                    "session_id": "some-session-id",
                }
            }))
            .unwrap()
            .into();

            let msg = Message::decode(data, false).unwrap();

            if let Message::ResumeACK(resume_ack) = msg {
                assert_eq!(resume_ack.data.session_id, "some-session-id");
            } else {
                panic!("decoded message is not reconnect")
            }
        }
    }

    mod encode {
        use super::super::*;

        #[test]
        fn test_message_encode_hello() {
            let msg = Message::Hello(OnlyData {
                data: Hello {
                    code: 0,
                    session_id: Some("some-session-id".to_string()),
                },
            });

            println!("{:?}", msg.encode());
        }
    }
}
