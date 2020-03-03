//! Common types used across the crate.

pub use http::Method as RequestMethod;
pub use http::StatusCode;
pub use http::Uri;

str_enum! {
    /// Represents the `filter_level` parameter in API requests.
    #[derive(Clone, Debug, PartialEq, Hash, Eq)]
    pub enum FilterLevel {
        None = "none",
        Low = "low",
        Medium = "medium",
    }
}

impl std::default::Default for FilterLevel {
    fn default() -> Self {
        FilterLevel::None
    }
}

impl std::fmt::Display for FilterLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        AsRef::<str>::as_ref(self).fmt(f)
    }
}
