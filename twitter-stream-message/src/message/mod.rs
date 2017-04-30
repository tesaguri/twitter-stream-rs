//! Messages from Streaming API.

mod event;
mod warning;

pub use self::event::{Event, EventKind};
pub use self::warning::{Warning, WarningCode};

use {DirectMessage, Error};
use serde::de::{Deserialize, Deserializer, Error as SerdeError, IgnoredAny, MapAccess, Unexpected, Visitor};
use std::fmt;
use std::str::FromStr;
use tweet::{StatusId, Tweet};
use types::{JsonMap, JsonValue};
use user::UserId;

/// Represents a message from Twitter Streaming API.
///
/// # Reference
///
/// 1. [Streaming message types — Twitter Developers](https://dev.twitter.com/streaming/overview/messages-types)
#[derive(Clone, Debug, PartialEq)]
pub enum StreamMessage {
    /// Tweet
    Tweet(Box<Tweet>),

    /// Notifications about non-Tweet events.
    Event(Box<Event>),

    /// Indicate that a given Tweet has been deleted.
    Delete(Delete),

    /// Indicate that geolocated data must be stripped from a range of Tweets.
    ScrubGeo(ScrubGeo),

    /// Indicate that a filtered stream has matched more Tweets than its current rate limit allows to be delivered,
    /// noticing a total count of the number of undelivered Tweets since the connection was opened.
    Limit(Limit),

    /// Indicate that a given tweet has had its content withheld.
    StatusWithheld(StatusWithheld),

    /// Indicate that a user has had their content withheld.
    UserWithheld(UserWithheld),

    /// This message is sent when a stream is disconnected, indicating why the stream was closed.
    Disconnect(Disconnect),

    /// Variout warning message
    Warning(Warning),

    /// List of the user's friends. Only be sent upon establishing a User Stream connection.
    Friends(Friends),

    // FriendsStr(Vec<String>), // TODO: deserialize `friends_str` into `Friends`

    /// Direct message
    DirectMessage(Box<DirectMessage>),

    /// A [control URI][1] for Site Streams.
    /// [1]: https://dev.twitter.com/streaming/sitestreams/controlstreams
    Control(Control),

    /// An [envelope][1] for Site Stream.
    /// [1]: https://dev.twitter.com/streaming/overview/messages-types#envelopes_for_user
    ForUser(UserId, Box<StreamMessage>),

    // ForUserStr(String, Box<StreamMessage>),

    /// A message not known to this library.
    Custom(JsonMap<String, JsonValue>),
}

/// Represents a deleted Tweet.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Delete {
    pub id: StatusId,
    pub user_id: UserId,
}

/// Represents a range of Tweets whose geolocated data must be stripped.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct ScrubGeo {
    pub user_id: UserId,
    pub up_to_status_id: StatusId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Limit {
    pub track: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct StatusWithheld {
    pub id: StatusId,
    pub user_id: UserId,
    pub withheld_in_countries: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct UserWithheld {
    pub id: UserId,
    pub withheld_in_countries: Vec<String>,
}

/// Indicates why a stream was closed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Disconnect {
    pub code: DisconnectCode,
    pub stream_name: String,
    pub reason: String,
}

macro_rules! number_enum {
    (
        $(#[$attr:meta])*
        pub enum $E:ident {
            $(
                $(#[$v_attr:meta])*
                :$V:ident = $n:expr,
            )*
        }
    ) => {
        $(#[$attr])*
        pub enum $E {
            $(
                $(#[$v_attr])*
                $V = $n,
            )*
        }

        impl<'x> Deserialize<'x> for $E {
            fn deserialize<D: Deserializer<'x>>(d: D) -> Result<Self, D::Error> {
                struct NEVisitor;

                impl<'x> Visitor<'x> for NEVisitor {
                    type Value = $E;

                    fn visit_u64<E: SerdeError>(self, v: u64) -> Result<$E, E> {
                        match v {
                            $($n => Ok($E::$V),)*
                            _ => Err(E::invalid_value(Unexpected::Unsigned(v), &self)),
                        }
                    }

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        write!(f, concat!("one of the following integers: ", $($n, ','),*))
                    }
                }

                d.deserialize_u64(NEVisitor)
            }
        }

        impl AsRef<str> for $E {
            fn as_ref(&self) -> &str {
                match *self {
                    $($E::$V => stringify!($V),)*
                }
            }
        }
    };
}

number_enum! {
    /// Status code for a `Disconnect` message.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    pub enum DisconnectCode {
        /// The feed was shutdown (possibly a machine restart).
        :Shutdown = 1,
        /// The same endpoint was connected too many times.
        :DuplicateStream = 2,
        /// Control streams was used to close a stream (applies to sitestreams).
        :ControlRequest = 3,
        /// The client was reading too slowly and was disconnected by the server.
        :Stall = 4,
        /// The client appeared to have initiated a disconnect.
        :Normal = 5,
        /// An oauth token was revoked for a user (applies to site and userstreams).
        :TokenRevoked = 6,
        /// The same credentials were used to connect a new stream and the oldest was disconnected.
        :AdminLogout = 7,
        // Reserved for internal use. Will not be delivered to external clients.
        // _ = 8,
        /// The stream connected with a negative count parameter and was disconnected after all backfill was delivered.
        :MaxMessageLimit = 9,
        /// An internal issue disconnected the stream.
        :StreamException = 10,
        /// An internal issue disconnected the stream.
        :BrokerStall = 11,
        /// The host the stream was connected to became overloaded and streams were disconnected to balance load.
        /// Reconnect as usual.
        :ShedLoad = 12,
    }
}

/// Represents a control message.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
pub struct Control {
    control_uri: String,
}

pub type Friends = Vec<UserId>;

impl<'x> Deserialize<'x> for StreamMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'x> {
        struct SMVisitor;

        impl<'x> Visitor<'x> for SMVisitor {
            type Value = StreamMessage;

            fn visit_map<V>(self, mut v: V) -> Result<StreamMessage, V::Error> where V: MapAccess<'x> {
                let key = match v.next_key::<String>()? {
                    Some(k) => k,
                    None => return Ok(StreamMessage::Custom(JsonMap::new())),
                };

                let ret = match key.as_str() {
                    "delete"          => Some(v.next_value().map(StreamMessage::Delete)),
                    "scrub_geo"       => Some(v.next_value().map(StreamMessage::ScrubGeo)),
                    "limit"           => Some(v.next_value().map(StreamMessage::Limit)),
                    "status_withheld" => Some(v.next_value().map(StreamMessage::StatusWithheld)),
                    "user_withheld"   => Some(v.next_value().map(StreamMessage::UserWithheld)),
                    "disconnect"      => Some(v.next_value().map(StreamMessage::Disconnect)),
                    "warning"         => Some(v.next_value().map(StreamMessage::Warning)),
                    "friends"         => Some(v.next_value().map(StreamMessage::Friends)),
                    // "friends_str"     => Some(v.next_value().map(StreamMessage::Friends)),
                    "direct_message"  => Some(v.next_value().map(StreamMessage::DirectMessage)),
                    "control"         => Some(v.next_value().map(StreamMessage::Control)),
                    _ => None,
                };

                if let Some(ret) = ret {
                    if ret.is_ok() {
                        while v.next_entry::<IgnoredAny,IgnoredAny>()?.is_some() {}
                    }
                    return ret;
                }

                // Tweet, Event or for_user envelope:

                let mut map = JsonMap::new();
                map.insert(key, v.next_value()?);
                while let Some((k, v)) = v.next_entry()? {
                    map.insert(k, v);
                }

                if map.contains_key("id") {
                    Tweet::deserialize(JsonValue::Object(map))
                        .map(Box::new)
                        .map(StreamMessage::Tweet)
                        .map_err(|e| V::Error::custom(e.to_string()))
                } else if map.contains_key("event") {
                    Event::deserialize(JsonValue::Object(map))
                        .map(Box::new)
                        .map(StreamMessage::Event)
                        .map_err(|e| V::Error::custom(e.to_string()))
                } else if let Some(id) = map.remove("for_user") {
                    if let Some(id) = id.as_u64() {
                        if let Some(msg) = map.remove("message") {
                            StreamMessage::deserialize(msg)
                                .map(|m| StreamMessage::ForUser(id, Box::new(m)))
                                .map_err(|e| V::Error::custom(e.to_string()))
                        } else {
                            Err(V::Error::missing_field("message"))
                        }
                    } else {
                        Err(V::Error::custom("expected u64"))
                    }
                } else {
                    Ok(StreamMessage::Custom(map))
                }
            }

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "map")
            }
        }

        deserializer.deserialize_map(SMVisitor)
    }
}

impl FromStr for StreamMessage {
    type Err = Error;

    fn from_str(json: &str) -> Result<Self, Error> {
        ::json::from_str(json)
    }
}

impl<'x> Deserialize<'x> for Delete {
    fn deserialize<D: Deserializer<'x>>(d: D) -> Result<Self, D::Error> {
        struct DeleteVisitor;

        impl<'x> Visitor<'x> for DeleteVisitor {
            type Value = Delete;

            fn visit_map<V: MapAccess<'x>>(self, mut v: V) -> Result<Delete, V::Error> {
                use std::mem;

                #[allow(dead_code)]
                #[derive(Deserialize)]
                struct Status { id: StatusId, user_id: UserId };

                while let Some(k) = v.next_key::<String>()? {
                    if "status" == k.as_str() {
                        let ret = v.next_value::<Status>()?;
                        while v.next_entry::<IgnoredAny,IgnoredAny>()?.is_some() {}
                        unsafe {
                            return Ok(mem::transmute(ret));
                        }
                    } else {
                        v.next_value::<IgnoredAny>()?;
                    }
                }

                Err(V::Error::missing_field("status"))
            }

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a map with a field `status` which contains field `id` and `user_id` of integer type`")
            }
        }

        d.deserialize_map(DeleteVisitor)
    }
}

impl fmt::Display for Disconnect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {} {}: {}", self.stream_name, self.code as u32, self.code.as_ref(), self.reason)
    }
}

#[cfg(test)]
mod tests {
    use json;
    use super::*;

    #[test]
    fn warning() {
        assert_eq!(
            StreamMessage::Warning(Warning {
                message: "Your connection is falling behind and messages are being queued for delivery to you. \
                    Your queue is now over 60% full. You will be disconnected when the queue is full.".to_owned(),
                code: WarningCode::FallingBehind(60),
            }),
            json::from_str(
                "{\"warning\":{\"code\":\"FALLING_BEHIND\",\"message\":\"Your connection is falling \
                    behind and messages are being queued for delivery to you. Your queue is now over 60% full. \
                    You will be disconnected when the queue is full.\",\"percent_full\": 60}}"
            ).unwrap()
        )
    }
}
