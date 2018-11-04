//! Common types used across the crate.

pub use hyper::Method as RequestMethod;
pub use hyper::StatusCode;
pub use hyper::Uri;

string_enums! {
    /// Represents the `filter_level` parameter in API requests.
    #[derive(Clone, Debug)]
    pub enum FilterLevel {
        None("none"),
        Low("low"),
        Medium("medium");
        Custom(_),
    }
}

impl std::default::Default for FilterLevel {
    fn default() -> Self {
        FilterLevel::None
    }
}
