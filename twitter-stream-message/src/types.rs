pub use json::Map as JsonMap;
pub use json::Number as JsonNumber;
pub use json::Value as JsonValue;

use chrono::{DateTime as ChronoDateTime, UTC};

string_enums! {
    /// Represents the `filter_level` field in Tweets.
    #[derive(Clone, Debug)]
    pub enum FilterLevel {
        :None("none"),
        :Low("low"),
        :Medium("medium");
        :Custom(_),
    }

    /// Represents the `withheld_scope` field in `Tweet` and `User`.
    #[derive(Clone, Debug)]
    pub enum WithheldScope {
        :Status("status"),
        :User("user");
        :Custom(_),
    }
}

pub type DateTime = ChronoDateTime<UTC>;
