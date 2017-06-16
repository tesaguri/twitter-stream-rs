//! Messages from Streaming API.

mod event;
mod warning;

pub use self::event::{Event, EventKind};
pub use self::warning::{Warning, WarningCode};

use DirectMessage;
use serde::de::{Deserialize, Deserializer, Error as SerdeError, IgnoredAny, MapAccess, Unexpected, Visitor};
use serde::de::value::MapAccessDeserializer;
use std::borrow::Cow;
use std::fmt;
use std::iter::{self, FromIterator};
use tweet::{StatusId, Tweet};
use types::{JsonMap, JsonValue};
use user::UserId;
use util::{CowStr, MapAccessChain};

/// Represents a message from Twitter Streaming API.
///
/// # Reference
///
/// 1. [Streaming message types — Twitter Developers](https://dev.twitter.com/streaming/overview/messages-types)
#[derive(Clone, Debug, PartialEq)]
pub enum StreamMessage<'a> {
    /// Tweet
    Tweet(Box<Tweet<'a>>),

    /// Notifications about non-Tweet events.
    Event(Box<Event<'a>>),

    /// Indicate that a given Tweet has been deleted.
    Delete(Delete),

    /// Indicate that geolocated data must be stripped from a range of Tweets.
    ScrubGeo(ScrubGeo),

    /// Indicate that a filtered stream has matched more Tweets than its current rate limit allows to be delivered,
    /// noticing a total count of the number of undelivered Tweets since the connection was opened.
    Limit(Limit),

    /// Indicate that a given tweet has had its content withheld.
    StatusWithheld(StatusWithheld<'a>),

    /// Indicate that a user has had their content withheld.
    UserWithheld(UserWithheld<'a>),

    /// This message is sent when a stream is disconnected, indicating why the stream was closed.
    Disconnect(Disconnect<'a>),

    /// Variout warning message
    Warning(Warning<'a>),

    /// List of the user's friends. Only be sent upon establishing a User Stream connection.
    Friends(Friends),

    // FriendsStr(Vec<String>), // TODO: deserialize `friends_str` into `Friends`

    /// Direct message
    DirectMessage(Box<DirectMessage<'a>>),

    /// A [control URI][1] for Site Streams.
    /// [1]: https://dev.twitter.com/streaming/sitestreams/controlstreams
    Control(Control<'a>),

    /// An [envelope][1] for Site Stream.
    /// [1]: https://dev.twitter.com/streaming/overview/messages-types#envelopes_for_user
    ForUser(UserId, Box<StreamMessage<'a>>),

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
pub struct StatusWithheld<'a> {
    pub id: StatusId,
    pub user_id: UserId,
    #[serde(borrow)]
    #[serde(deserialize_with = "::util::deserialize_vec_cow_str")]
    pub withheld_in_countries: Vec<Cow<'a, str>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct UserWithheld<'a> {
    pub id: UserId,
    #[serde(borrow)]
    #[serde(deserialize_with = "::util::deserialize_vec_cow_str")]
    pub withheld_in_countries: Vec<Cow<'a, str>>,
}

/// Indicates why a stream was closed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Disconnect<'a> {
    pub code: DisconnectCode,

    #[serde(borrow)]
    pub stream_name: Cow<'a, str>,

    #[serde(borrow)]
    pub reason: Cow<'a, str>,
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
pub struct Control<'a> {
    #[serde(borrow)]
    control_uri: Cow<'a, str>,
}

pub type Friends = Vec<UserId>;

impl<'a> StreamMessage<'a> {
    /// Parse a JSON string returned from Twitter Streaming API.
    ///
    /// Note that this method is not a member of the `FromStr` trait.
    /// It is because the method requires the lifetime information of the JSON string,
    /// while `FromStr::from_str` does not take a lifetime parameter.
    ///
    /// ```
    /// use twitter_stream_message::message::{Delete, StreamMessage};
    ///
    /// let parsed = StreamMessage::from_str(r#"{
    ///     "delete":{
    ///         "status":{
    ///             "id":1234,
    ///             "id_str":"1234",
    ///             "user_id":3,
    ///             "user_id_str":"3"
    ///         }
    ///     }
    /// }"#).unwrap();
    /// let expected = StreamMessage::Delete(Delete {
    ///     id: 1234,
    ///     user_id: 3,
    /// });
    ///
    /// assert_eq!(parsed, expected);
    pub fn from_str(json: &'a str) -> ::Result<Self> {
        ::json::from_str(json)
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for StreamMessage<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        struct SMVisitor;

        impl<'a> Visitor<'a> for SMVisitor {
            type Value = StreamMessage<'a>;

            fn visit_map<A>(self, mut a: A) -> Result<StreamMessage<'a>, A::Error> where A: MapAccess<'a> {
                let mut key = match a.next_key::<CowStr>()? {
                    Some(k) => k,
                    None => return Ok(StreamMessage::Custom(JsonMap::new())),
                };

                let ret = match key.as_ref() {
                    "delete"          => Some(a.next_value().map(StreamMessage::Delete)),
                    "scrub_geo"       => Some(a.next_value().map(StreamMessage::ScrubGeo)),
                    "limit"           => Some(a.next_value().map(StreamMessage::Limit)),
                    "status_withheld" => Some(a.next_value().map(StreamMessage::StatusWithheld)),
                    "user_withheld"   => Some(a.next_value().map(StreamMessage::UserWithheld)),
                    "disconnect"      => Some(a.next_value().map(StreamMessage::Disconnect)),
                    "warning"         => Some(a.next_value().map(StreamMessage::Warning)),
                    "friends"         => Some(a.next_value().map(StreamMessage::Friends)),
                    // "friends_str"     => Some(a.next_value().map(StreamMessage::Friends)),
                    "direct_message"  => Some(a.next_value().map(StreamMessage::DirectMessage)),
                    "control"         => Some(a.next_value().map(StreamMessage::Control)),
                    _ => None,
                };

                if let Some(ret) = ret {
                    if ret.is_ok() {
                        while a.next_entry::<IgnoredAny,IgnoredAny>()?.is_some() {}
                    }
                    return ret;
                }

                // Tweet, Event or for_user envelope:

                let mut keys = Vec::new();
                let mut vals = Vec::new();

                loop {
                    match key.as_ref() {
                        "id" => {
                            let a = MapAccessChain::new(keys.into_iter().chain(iter::once(key.0)), vals, a);
                            return Tweet::deserialize(MapAccessDeserializer::new(a))
                                .map(Box::new)
                                .map(StreamMessage::Tweet);
                        },
                        "event" => {
                            let a = MapAccessChain::new(keys.into_iter().chain(iter::once(key.0)), vals, a);
                            return Event::deserialize(MapAccessDeserializer::new(a))
                                .map(Box::new)
                                .map(StreamMessage::Event);
                        },
                        "for_user" => {
                            let id = a.next_value::<u64>()?;
                            if let Some((_, v)) = keys.iter().zip(vals).find(|&(k, _)| "message" == k.as_ref()) {
                                let ret = StreamMessage::deserialize(v)
                                    .map(|m| StreamMessage::ForUser(id, Box::new(m)))
                                    .map_err(A::Error::custom)?;
                                while a.next_entry::<IgnoredAny,IgnoredAny>()?.is_some() {}
                                return Ok(ret);
                            } else {
                                loop {
                                    if let Some(k) = a.next_key::<CowStr>()? {
                                        if "message" == k.as_ref() {
                                            let ret = a.next_value()
                                                .map(|m| StreamMessage::ForUser(id, Box::new(m)))?;
                                            while a.next_entry::<IgnoredAny,IgnoredAny>()?.is_some() {}
                                            return Ok(ret);
                                        }
                                    } else {
                                        break;
                                    }
                                    a.next_value::<IgnoredAny>()?;
                                }

                                return Err(A::Error::missing_field("message"));
                            }
                        },
                        _ => {
                            keys.push(key.0);
                            vals.push(a.next_value()?);
                            key = if let Some(k) = a.next_key()? {
                                k
                            } else {
                                return Ok(StreamMessage::Custom(
                                    JsonMap::from_iter(keys.into_iter().map(Cow::into_owned).zip(vals))
                                ));
                            };
                        },
                    }
                }
            }

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a map")
            }
        }

        deserializer.deserialize_map(SMVisitor)
    }
}

impl<'x> Deserialize<'x> for Delete {
    fn deserialize<D: Deserializer<'x>>(d: D) -> Result<Self, D::Error> {
        struct DeleteVisitor;

        impl<'x> Visitor<'x> for DeleteVisitor {
            type Value = Delete;

            fn visit_map<A: MapAccess<'x>>(self, mut a: A) -> Result<Delete, A::Error> {
                use std::mem;

                #[allow(dead_code)]
                #[derive(Deserialize)]
                struct Status { id: StatusId, user_id: UserId };

                while let Some(k) = a.next_key::<CowStr>()? {
                    if "status" == k.as_ref() {
                        let ret = a.next_value::<Status>()?;
                        while a.next_entry::<IgnoredAny,IgnoredAny>()?.is_some() {}
                        unsafe {
                            return Ok(mem::transmute(ret));
                        }
                    } else {
                        a.next_value::<IgnoredAny>()?;
                    }
                }

                Err(A::Error::missing_field("status"))
            }

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a map with a field `status` which contains field `id` and `user_id` of integer type`")
            }
        }

        d.deserialize_map(DeleteVisitor)
    }
}

impl<'a> fmt::Display for Disconnect<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {} {}: {}", self.stream_name, self.code as u32, self.code.as_ref(), self.reason)
    }
}

#[cfg(test)]
mod tests {
    use json;
    use super::*;

    #[test]
    fn parse() {
        json::from_str::<StreamMessage>(
            r#"{
                "created_at":"Mon May 01 00:00:00 +0000 2017",
                "id":123456789012345678,
                "id_str":"123456789012345678",
                "text":"https:\/\/t.co\/iLy2wQAbv6 via @twitterapi",
                "source":"\u003ca href=\"http:\/\/twitter.com\/download\/iphone\" rel=\"nofollow\"\u003eTwitter for iPhone\u003c\/a\u003e",
                "truncated":false,
                "in_reply_to_status_id":null,
                "in_reply_to_status_id_str":null,
                "in_reply_to_user_id":null,
                "in_reply_to_user_id_str":null,
                "in_reply_to_screen_name":null,
                "user":{
                    "id":1234567890,
                    "id_str":"1234567890",
                    "name":"test",
                    "screen_name":"test",
                    "location":null,
                    "url":null,
                    "description":null,
                    "protected":false,
                    "verified":false,
                    "followers_count":9999,
                    "friends_count":100,
                    "listed_count":10,
                    "favourites_count":10000,
                    "statuses_count":12345,
                    "created_at":"Thu Jan 01 12:34:56 +0000 2015",
                    "utc_offset":null,
                    "time_zone":null,
                    "geo_enabled":true,
                    "lang":"en",
                    "contributors_enabled":false,
                    "is_translator":false,
                    "profile_background_color":"F5F8FA",
                    "profile_background_image_url":"",
                    "profile_background_image_url_https":"",
                    "profile_background_tile":false,
                    "profile_link_color":"1DA1F2",
                    "profile_sidebar_border_color":"C0DEED",
                    "profile_sidebar_fill_color":"DDEEF6",
                    "profile_text_color":"333333",
                    "profile_use_background_image":true,
                    "profile_image_url":
                        "http:\/\/abs.twimg.com\/sticky\/default_profile_images\/default_profile_normal.png",
                    "profile_image_url_https":
                        "https:\/\/abs.twimg.com\/sticky\/default_profile_images\/default_profile_normal.png",
                    "default_profile":true,
                    "default_profile_image":true,
                    "following":null,
                    "follow_request_sent":null,
                    "notifications":null
                },
                "geo":null,
                "coordinates":null,
                "place":{
                    "id":"1ae56642f20314ee",
                    "url":"https:\/\/api.twitter.com\/1.1\/geo\/id\/1ae56642f20314ee.json",
                    "place_type":"city",
                    "name":"Mito-shi",
                    "full_name":"Mito-shi, Ibaraki",
                    "country_code":"JP",
                    "country":"Japan",
                    "contained_within":[],
                    "bounding_box":{
                        "type":"Polygon",
                        "coordinates":[[
                            [140.321777,36.300635],[140.586127,36.300635],[140.586127,36.464512],[140.321777,36.464512]
                        ]]
                    },
                    "attributes":{}
                },
                "contributors":null,
                "is_quote_status":false,
                "retweet_count":0,
                "favorite_count":0,
                "entities":{
                    "hashtags":[],
                    "urls":[{
                        "url":"https:\/\/t.co\/iLy2wQAbv6",
                        "expanded_url":"https:\/\/twittercommunity.com\/t\/retiring-legacy-dm-commands-on-the-standard-tweet-api\/86653",
                        "display_url":"twittercommunity.com\/t\/retiring-leg\u2026",
                        "indices":[0,23]
                    }],
                    "user_mentions":[{
                        "screen_name":"twitterapi",
                        "name":"Twitter API",
                        "id":6253282,
                        "id_str":"6253282",
                        "indices":[28,39]
                    }],
                    "symbols":[]
                },
                "favorited":false,
                "retweeted":false,
                "possibly_sensitive":false,
                "filter_level":"low",
                "lang":"en",
                "timestamp_ms":"1493564400000"
            }"#
        ).unwrap();
    }

    #[test]
    fn warning() {
        assert_eq!(
            StreamMessage::Warning(Warning {
                message: "Your connection is falling behind and messages are being queued for delivery to you. \
                    Your queue is now over 60% full. You will be disconnected when the queue is full.".into(),
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
