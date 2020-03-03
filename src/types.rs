//! Common types used across the crate.

pub use http::Method as RequestMethod;
pub use http::StatusCode;
pub use http::Uri;

/// A `BoundingBox` is a rectangular area on the globe specified by coordinates of
/// the southwest and northeast edges in decimal degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct BoundingBox {
    /// Longitude of the west side of the bounding box.
    pub west_longitude: f64,
    /// Latitude of the south side of the bounding box.
    pub south_latitude: f64,
    /// Longitude of the east side of the bounding box.
    pub east_longitude: f64,
    /// Latitude of the north side of the bounding box.
    pub north_latitude: f64,
}

str_enum! {
    /// Represents the `filter_level` parameter in API requests.
    #[derive(Clone, Debug, PartialEq, Hash, Eq)]
    pub enum FilterLevel {
        None = "none",
        Low = "low",
        Medium = "medium",
    }
}

impl BoundingBox {
    /// Creates a `BoundingBox` with two `(longitude, latitude)` pairs.
    ///
    /// The first argument specifies the southwest edge and the second specifies the northeast edge.
    ///
    /// # Example
    ///
    /// ```rust
    /// use twitter_stream::types::BoundingBox;
    ///
    /// // Examples taken from Twitter's documentation.
    /// BoundingBox::new((-122.75, 36.8), (-121.75,37.8)); // San Francisco
    /// BoundingBox::new((-74.0, 40.0), (-73.0, 41.0)); // New York City
    /// ```
    pub fn new(
        (west_longitude, south_latitude): (f64, f64),
        (east_longitude, north_latitude): (f64, f64),
    ) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
    }
}

impl From<(f64, f64, f64, f64)> for BoundingBox {
    fn from(
        (west_longitude, south_latitude, east_longitude, north_latitude): (f64, f64, f64, f64),
    ) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
    }
}

impl From<((f64, f64), (f64, f64))> for BoundingBox {
    fn from(
        ((west_longitude, south_latitude), (east_longitude, north_latitude)): (
            (f64, f64),
            (f64, f64),
        ),
    ) -> Self {
        BoundingBox {
            west_longitude,
            south_latitude,
            east_longitude,
            north_latitude,
        }
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
