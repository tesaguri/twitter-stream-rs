use serde::de::{Deserialize, Deserializer, Error, MapVisitor, Visitor};
use std::collections::HashMap;
use super::{DateTime, Entities, Geometry, Place, User, UserId};
use json::Value;

macro_rules! def_tweet {
    (
        $(#[$attr:meta])*
        pub struct $T:ident {
            $(
                $(#[$f_attr:meta])*
                pub $field:ident: $f_ty:ty
            ),*;
            $(
                $(#[$op_attr:meta])*
                pub $op_field:ident: $op_ty:ty
            ),*
        }
    ) => {
        $(#[$attr])*
        pub struct $T {
            $(
                $(#[$f_attr])*
                pub $field: $f_ty,
            )*
            pub created_at: DateTime,
            $(
                $(#[$op_attr])*
                pub $op_field: Option<$op_ty>,
            )*
            pub current_user_retweet: Option<StatusId>,
        }

        impl Deserialize for $T {
            fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
                struct TweetVisitor;

                impl Visitor for TweetVisitor {
                    type Value = $T;

                    fn visit_map<V: MapVisitor>(&mut self, mut v: V) -> Result<$T, V::Error> {
                        #[derive(Default)]
                        struct TweetReader {
                            $($field: Option<$f_ty>,)*
                            created_at: Option<DateTime>,
                            $($op_field: Option<$op_ty>,)*
                            current_user_retweet: Option<StatusId>,
                        }

                        let mut t = TweetReader::default();

                        while let Some(k) = v.visit_key::<String>()? {
                            match k.as_str() {
                                $(stringify!($field) => t.$field = Some(v.visit_value()?),)*
                                "created_at" => t.created_at = {
                                    let s = v.visit_value::<String>()?;
                                    Some(super::parse_datetime(&s).map_err(|e| V::Error::custom(e.to_string()))?)
                                },
                                $(stringify!($op_field) => t.$op_field = v.visit_value()?,)*
                                "current_user_retweet" => t.current_user_retweet = {
                                    struct IdObject(UserId);
                                    impl Deserialize for IdObject {
                                        fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
                                            struct IdVisitor;
                                            impl Visitor for IdVisitor {
                                                type Value = IdObject;
                                                fn visit_map<V>(&mut self, mut v: V) -> Result<IdObject, V::Error>
                                                    where V: MapVisitor
                                                {
                                                    let mut id = None;
                                                    while let Some(k) = v.visit_key::<String>()? {
                                                        if "id" == k.as_str() {
                                                            id = Some(v.visit_value()?);
                                                        }
                                                    }
                                                    if let Some(id) = id {
                                                        Ok(IdObject(id))
                                                    } else {
                                                        v.missing_field("id")
                                                    }
                                                }
                                            }
                                            d.deserialize_map(IdVisitor)
                                        }
                                    }
                                    Some(v.visit_value::<IdObject>()?.0)
                                },
                                _ => (),
                            }
                        }

                        if let TweetReader {
                                $($field: Some($field),)*
                                created_at: Some(ca),
                                $($op_field,)*
                                current_user_retweet,
                            } = t
                        {
                            Ok($T {
                                $($field: $field,)*
                                created_at: ca,
                                $($op_field: $op_field,)*
                                current_user_retweet: current_user_retweet,
                            })
                        } else {
                            $(
                                if t.$field.is_none() {
                                    return v.missing_field(stringify!($field));
                                }
                            )*
                            unreachable!();
                        }
                    }
                }

                d.deserialize_map(TweetVisitor)
            }
        }
    };
}

def_tweet! {
    #[derive(Clone, Debug, PartialEq)]
    pub struct Tweet {
        // pub created_at: DateTime, // Implemented inside `def_tweet!` macro.
        pub entities: Entities,
        pub id: StatusId,
        pub retweet_count: u64,
        pub retweeted: bool,
        pub source: String,
        pub text: String,
        pub truncated: bool,
        pub user: User;

        pub coordinates: Geometry,
        // // Implemented inside `def_tweet!` macro. An optional value cannot be simply deserialized with
        // // `#[serde(deserialize_with = "...")]` attribute because it cannot handle absence of the value.
        // pub current_user_retweet: StatusId,
        pub favorite_count: u64,
        pub favorited: bool,
        pub filter_level: FilterLevel,
        pub in_reply_to_screen_name: String,
        pub in_reply_to_status_id: String,
        pub in_reply_to_user_id: UserId,
        pub lang: String,
        pub place: Place,
        pub possibly_sensitive: bool,
        pub quoted_status_id: StatusId,
        pub quoted_status: Box<Tweet>,
        pub scopes: Option<HashMap<String, Value>>,
        pub withheld_copyright: bool,
        pub withheld_in_countries: Vec<String>,
        pub withheld_scope: String
    }
}

string_enums! {
    #[derive(Clone, Debug)]
    pub enum FilterLevel {
        None("none"),
        Low("low"),
        Medium("medium");
        Unknown(_),
    }
}

pub type StatusId = u64;
