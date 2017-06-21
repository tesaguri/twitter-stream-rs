//! Common types used across the crate.

pub use json::Map as JsonMap;
pub use json::Number as JsonNumber;
pub use json::Value as JsonValue;

use chrono::{DateTime as ChronoDateTime, Utc};

string_enums! {
    /// Represents the `filter_level` field in Tweets.
    #[derive(Clone, Debug)]
    pub enum FilterLevel<'a> {
        :None("none"),
        :Low("low"),
        :Medium("medium");
        :Custom(_),
    }

    /// Represents the `withheld_scope` field in `Tweet` and `User`.
    #[derive(Clone, Debug)]
    pub enum WithheldScope<'a> {
        :Status("status"),
        :User("user");
        :Custom(_),
    }
}

pub type DateTime = ChronoDateTime<Utc>;
