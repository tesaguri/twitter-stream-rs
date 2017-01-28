//! Message from Streaming API.

mod event;
mod warning;

pub use self::event::{Event, EventKind};
pub use self::warning::{Warning, WarningCode};

use serde::de::{Deserialize, Deserializer, Error, MapVisitor, Visitor};
use serde::de::impls::IgnoredAny;
use std::fmt;
use super::{DirectMessage, StatusId, Tweet, UserId};
use json::value::{Deserializer as JsonDeserializer, Map, Value};

/// Represents a message from Twitter Streaming API.
///
/// # Reference
///
/// 1. [Streaming message types — Twitter Developers](https://dev.twitter.com/streaming/overview/messages-types)
#[derive(Clone, Debug, PartialEq)]
pub enum StreamMessage {
    /// Tweet
    Tweet(Tweet),

    /// Notifications about non-Tweet events.
    Event(Event),

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
    DirectMessage(DirectMessage),

    /// A [control URI][1] for Site Streams.
    /// [1]: https://dev.twitter.com/streaming/sitestreams/controlstreams
    Control(Control), 

    /// An [envelope][1] for Site Stream.
    /// [1]: https://dev.twitter.com/streaming/overview/messages-types#envelopes_for_user
    ForUser(UserId, Box<StreamMessage>),

    // ForUserStr(String, Box<StreamMessage>),

    /// A message not known to this library.
    Custom(Map<String, Value>),
}

/// Represents a deleted Tweet.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Delete {
    pub id: StatusId,
    pub user_id: UserId,
}

pub use serde_types::stream::*;

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

        impl ::serde::Deserialize for $E {
            fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
                struct Visitor;

                impl ::serde::de::Visitor for Visitor {
                    type Value = $E;

                    fn visit_u64<E: Error>(&mut self, v: u64) -> Result<$E, E> {
                        match v {
                            $($n => Ok($E::$V),)*
                            _ => Err(E::invalid_value(&v.to_string())),
                        }
                    }
                }

                d.deserialize_u64(Visitor)
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

pub type Friends = Vec<UserId>;

impl Deserialize for StreamMessage {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error> where D: Deserializer {
        struct SMVisitor;

        impl Visitor for SMVisitor {
            type Value = StreamMessage;

            fn visit_map<V>(&mut self, mut v: V) -> Result<StreamMessage, V::Error> where V: MapVisitor {
                let key = match v.visit_key::<String>()? {
                    Some(k) => k,
                    None => {
                        v.end()?;
                        return Ok(StreamMessage::Custom(Map::new()));
                    },
                };

                let ret = match key.as_str() {
                    "delete"          => Some(v.visit_value().map(StreamMessage::Delete)),
                    "scrub_geo"       => Some(v.visit_value().map(StreamMessage::ScrubGeo)),
                    "limit"           => Some(v.visit_value().map(StreamMessage::Limit)),
                    "status_withheld" => Some(v.visit_value().map(StreamMessage::StatusWithheld)),
                    "user_withheld"   => Some(v.visit_value().map(StreamMessage::UserWithheld)),
                    "disconnect"      => Some(v.visit_value().map(StreamMessage::Disconnect)),
                    "warning"         => Some(v.visit_value().map(StreamMessage::Warning)),
                    "friends"         => Some(v.visit_value().map(StreamMessage::Friends)),
                    // "friends_str"     => Some(v.visit_value().map(StreamMessage::Friends)),
                    "direct_message"  => Some(v.visit_value().map(StreamMessage::DirectMessage)),
                    "control"         => Some(v.visit_value().map(StreamMessage::Control)),
                    _ => None,
                };

                if let Some(ret) = ret {
                    if ret.is_ok() {
                        while v.visit::<IgnoredAny,IgnoredAny>()?.is_some() {}
                        v.end()?;
                    }
                    return ret;
                }

                // Tweet, Event or for_user envelope:

                let mut map = Map::new();
                map.insert(key, v.visit_value()?);
                while let Some((k, v)) = v.visit()? {
                    map.insert(k, v);
                }
                v.end()?;

                if map.contains_key("id") {
                    let mut d = JsonDeserializer::new(Value::Object(map));
                    Tweet::deserialize(&mut d)
                        .map(StreamMessage::Tweet)
                        .map_err(|e| V::Error::custom(e.to_string()))
                } else if map.contains_key("event") {
                    let mut d = JsonDeserializer::new(Value::Object(map));
                    Event::deserialize(&mut d)
                        .map(StreamMessage::Event)
                        .map_err(|e| V::Error::custom(e.to_string()))
                } else if let Some(id) = map.remove("for_user") {
                    if let Value::U64(id) = id {
                        if let Some(m) = map.remove("message") {
                            let mut d = JsonDeserializer::new(m);
                            StreamMessage::deserialize(&mut d)
                                .map(|m| StreamMessage::ForUser(id, Box::new(m)))
                                .map_err(|e| V::Error::custom(e.to_string()))
                        } else {
                            v.missing_field("message")
                        }
                    } else {
                        Err(V::Error::custom("expected u64"))
                    }
                } else {
                    Ok(StreamMessage::Custom(map))
                }
            }
        }

        deserializer.deserialize_map(SMVisitor)
    }
}

impl Deserialize for Delete {
    fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
        struct DeleteVisitor;

        impl Visitor for DeleteVisitor {
            type Value = Delete;

            fn visit_map<V: MapVisitor>(&mut self, mut v: V) -> Result<Delete, V::Error> {
                use serde_types::stream_delete::Status;

                while let Some(k) = v.visit_key::<String>()? {
                    match k.as_str() {
                        "status" => {
                            let ret = unsafe { ::std::mem::transmute::<_,_>(v.visit_value::<Status>()?) };
                            while let Some(_) = v.visit::<IgnoredAny,IgnoredAny>()? {}
                            v.end()?;
                            return Ok(ret);
                        },
                        _ => { v.visit_value::<IgnoredAny>()?; },
                    }
                }

                v.missing_field("status")
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
