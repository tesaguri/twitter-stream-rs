//! Type-safe abstractions over JSON messages from Twitter Stream API.

macro_rules! string_enums {
    (
        $(
            $(#[$attr:meta])*
            pub enum $E:ident {
                $(
                    $(#[$v_attr:meta])*
                    :$V:ident($by:expr) // The leading (ugly) colon is to suppress local ambiguity error.
                ),*;
                $(#[$u_attr:meta])*
                :$U:ident(_),
            }
        )*
    ) => {
        $(
            $(#[$attr])*
            pub enum $E {
                $(
                    $(#[$v_attr])*
                    $V,
                )*
                $(#[$u_attr])*
                $U(String),
            }

            impl ::serde::Deserialize for $E {
                fn deserialize<D: ::serde::Deserializer>(d: &mut D) -> ::std::result::Result<Self, D::Error> {
                    struct V;

                    impl ::serde::de::Visitor for V {
                        type Value = $E;

                        fn visit_str<E>(&mut self, s: &str) -> ::std::result::Result<$E, E> {
                            match s {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s.to_owned())),
                            }
                        }

                        fn visit_string<E>(&mut self, s: String) -> ::std::result::Result<$E, E> {
                            match s.as_str() {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s)),
                            }
                        }
                    }

                    d.deserialize_string(V)
                }
            }

            impl ::std::convert::AsRef<str> for $E {
                fn as_ref(&self) -> &str {
                    match *self {
                        $($E::$V => $by,)*
                        $E::$U(ref s) => s,
                    }
                }
            }

            impl ::std::cmp::PartialEq for $E {
                fn eq(&self, other: &$E) -> bool {
                    match *self {
                        $($E::$V => match *other {
                            $E::$V => true,
                            $E::$U(ref s) if $by == s => true,
                            _ => false,
                        },)*
                        $E::$U(ref s) => match *other {
                            $($E::$V => $by == s,)*
                            $E::$U(ref t) => s == t,
                        },
                    }
                }
            }

            impl ::std::hash::Hash for $E {
                fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                    match *self {
                        $($E::$V => $by.hash(state),)*
                        $E::$U(ref s) => s.hash(state),
                    }
                }
            }

            impl ::std::cmp::Eq for $E {}
        )*
    }
}

pub mod direct_message;
pub mod entities;
pub mod geometry;
pub mod list;
pub mod place;
pub mod stream;
pub mod tweet;
pub mod user;

pub use self::direct_message::{DirectMessage, DirectMessageId};
pub use self::entities::Entities;
pub use self::geometry::Geometry;
pub use self::list::{List, ListId};
pub use self::place::Place;
pub use self::stream::StreamMessage;
pub use self::tweet::{StatusId, Tweet};
pub use self::user::{User, UserId};

use chrono::{self, DateTime as ChronoDateTime, TimeZone, UTC};
use serde::de::{Deserialize, Deserializer, Error};

string_enums! {
    /// Represents the `filter_level` field in Tweets or `filter_level` parameter in API request.
    #[derive(Clone, Debug)]
    pub enum FilterLevel {
        :None("none"),
        :Low("low"),
        :Medium("medium");
        :Custom(_),
    }
}

string_enums! {
    /// Represents the `withheld_scope` field in `Tweet` and `User`.
    #[derive(Clone, Debug)]
    pub enum WithheldScope {
        :Status("status"),
        :User("user");
        :Custom(_),
    }
}

pub type DateTime = ChronoDateTime<UTC>;

impl ::std::default::Default for FilterLevel {
    fn default() -> Self {
        FilterLevel::None
    }
}

fn parse_datetime(s: &str) -> chrono::format::ParseResult<DateTime> {
    UTC.datetime_from_str(s, "%a %b %e %H:%M:%S %z %Y")
}

fn deserialize_datetime<D: Deserializer>(d: &mut D) -> Result<DateTime, D::Error> {
    parse_datetime(&String::deserialize(d)?).map_err(|e| D::Error::custom(e.to_string()))
}
