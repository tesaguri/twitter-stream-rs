//! Common types used across the crate.

pub use hyper::method::Method as RequestMethod;
pub use hyper::status::StatusCode;
pub use json::Map as JsonMap;
pub use json::Number as JsonNumber;
pub use json::Value as JsonValue;

use chrono::{DateTime as ChronoDateTime, UTC};

string_enums! {
    /// Represents the `filter_level` field in Tweets or `filter_level` parameter in API request.
    #[derive(Clone, Debug)]
    pub enum FilterLevel {
        :None("none"),
        :Low("low"),
        :Medium("medium");
        :Custom(_),
    }

    /// A value for `with` parameter for User and Site Streams.
    #[derive(Clone, Debug)]
    pub enum With {
        /// Instruct the stream to send messages only from the user associated with that stream.
        /// The default for Site Streams.
        :User("user"),
        /// Instruct the stream to send messages from accounts the user follows as well, equivalent
        /// to the user’s home timeline. The default for User Streams.
        :Following("following");
        /// Custom value.
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

impl ::std::default::Default for FilterLevel {
    fn default() -> Self {
        FilterLevel::None
    }
}
