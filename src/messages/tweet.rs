use serde::de::{Deserializer, MapVisitor, Visitor};
use serde::de::impls::IgnoredAny;
use std::collections::HashMap;
use super::{DateTime, Entities, FilterLevel, Geometry, Place, User, UserId};
use json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Tweet {
    // pub contributors: Option<_>, // deprecated
    pub coordinates: Option<Geometry>,
    #[serde(deserialize_with = "super::deserialize_datetime")]
    pub created_at: DateTime,
    #[serde(deserialize_with = "current_user_retweet")]
    #[serde(default)]
    pub current_user_retweet: Option<StatusId>,
    pub entities: Entities,
    pub favorite_count: Option<u64>,
    pub favorited: Option<bool>,
    pub filter_level: Option<FilterLevel>,
    // pub geo: _, // deprecated in favor of `coordinates` field.
    pub id: StatusId,
    // pub id_str: String,
    pub in_reply_to_screen_name: Option<String>,
    pub in_reply_to_status_id: Option<StatusId>,
    // pub in_reply_to_status_id_str: Option<String>,
    pub in_reply_to_user_id: Option<UserId>,
    // pub in_reply_to_user_id_str: Option<String>,
    pub is_quote_status: bool,
    pub lang: Option<String>,
    pub place: Option<Place>,
    pub possibly_sensitive: Option<bool>,
    pub quoted_status_id: Option<StatusId>,
    // pub quoted_status_id_str: Option<String>,
    pub quoted_status: Option<Box<Tweet>>,
    pub scopes: Option<HashMap<String, Value>>,
    pub retweet_count: u64,
    pub retweeted: bool,
    pub retweeted_status: Option<Box<Tweet>>,
    pub source: String,
    pub text: String,
    pub truncated: bool,
    pub user: User,
    #[serde(default)]
    pub withheld_copyright: bool,
    #[serde(default)]
    pub withheld_in_countries: Vec<String>,
    pub withheld_scope: Option<WithheldScope>,
}

string_enums! {
    #[derive(Clone, Debug)]
    pub enum WithheldScope {
        Status("status"),
        User("user");
        Custom(_),
    }
}

pub type StatusId = u64;

fn current_user_retweet<D: Deserializer>(d: &mut D) -> Result<Option<StatusId>, D::Error> {
    struct CURVisitor;

    impl Visitor for CURVisitor {
        type Value = Option<StatusId>;

        fn visit_map<V: MapVisitor>(&mut self, mut v: V) -> Result<Option<StatusId>, V::Error> {
            while let Some(k) = v.visit_key::<String>()? {
                if "id" == k {
                    let ret = v.visit_value()?;
                    while let Some(_) = v.visit::<IgnoredAny, IgnoredAny>()? {}
                    v.end()?;
                    return Ok(Some(ret));
                } else {
                    v.visit_value::<IgnoredAny>()?;
                }
            }
            v.end()?;
            v.missing_field("id")
        }
    }

    d.deserialize_map(CURVisitor)
}
