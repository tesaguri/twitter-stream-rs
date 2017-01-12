// https://dev.twitter.com/streaming/overview/messages-types

pub mod event;

mod warning;

pub use self::event::Event;
pub use self::warning::{Warning, WarningCode};

use serde::de::{Deserialize, Deserializer, Error, MapVisitor, Visitor};
use serde::de::impls::IgnoredAny;
use super::{DirectMessage, List, StatusId, Tweet, User, UserId};
use json::value::{Deserializer as JsonDeserializer, Map, Value};

#[derive(Clone, Debug, PartialEq)]
pub enum StreamMessage {
    Tweet(Tweet),
    Event(Event),
    Delete(Delete),
    ScrubGeo(ScrubGeo),
    Limit(Limit),
    StatusWithheld(StatusWithheld),
    UserWithheld(UserWithheld),
    Disconnect(Disconnect),
    Warning(Warning),
    Friends(Friends),
    FriendsStr(Vec<String>),
    DirectMessage(DirectMessage),
    Control(Control),
    ForUser(UserId, Box<StreamMessage>),
    ForUserStr(String, Box<StreamMessage>),
    Custom(Map<String, Value>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Delete {
    pub id: StatusId,
    pub user_id: UserId,
}

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Disconnect {
    pub code: DisconnectCode,
    pub stream_name: String,
    pub reason: String,
}

macro_rules! number_enum {
    (
        pub enum $E:ident {
            $(
                $(#[$attr:meta])*
                $V:ident = $n:expr,
            )*
        }
    ) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        pub enum $E {
            $(
                $(#[$attr])*
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
    };
}

number_enum! {
    pub enum DisconnectCode {
        Shutdown = 1,
        DuplicateStream = 2,
        ControlRequest = 3,
        Stall = 4,
        Normal = 5,
        TokenRevoked = 6,
        AdminLogout = 7,
        // Reserved = 8,
        MaxMessageLimit = 9,
        StreamException = 10,
        BrokerStall = 11,
        ShedLoad = 12,
    }
}

pub type Friends = Vec<UserId>;
pub type FriendsStr = Vec<String>;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
pub struct Control {
    control_uri: String,
}

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
                    "friends_str"     => Some(v.visit_value().map(StreamMessage::FriendsStr)),
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
                } else if let Some(id) = map.remove("for_user") {
                    if let Value::String(id) = id {
                        if let Some(m) = map.remove("message") {
                            let mut d = JsonDeserializer::new(m);
                            StreamMessage::deserialize(&mut d)
                                .map(|m| StreamMessage::ForUserStr(id, Box::new(m)))
                                .map_err(|e| V::Error::custom(e.to_string()))
                        } else {
                            v.missing_field("message")
                        }
                    } else {
                        Err(V::Error::custom("expected String"))
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
                #[allow(dead_code)]
                #[derive(Deserialize)]
                struct Status { id: StatusId, user_id: UserId };

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
