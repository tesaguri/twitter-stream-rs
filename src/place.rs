//! Place

use geometry::Geometry;
use std::collections::HashMap;

/// Represents `place` field in `Tweet`.
///
/// # Reference
///
/// 1. [Places — Twitter Developers](https://dev.twitter.com/overview/api/places)
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Place {
    /// Contains a hash of variant information about the place. See [Place Attributes][1] for more detail.
    /// [1]: https://dev.twitter.com/overview/api/places#place_attributes
    pub attributes: Attributes,

    /// A bounding box of coordinates which encloses this place.
    pub bounding_box: Geometry,

    /// Name of the country containing this place.
    pub country: String,

    /// Shortened country code representing the country containing this place.
    pub country_code: String,

    /// Full human-readable representation of the place’s name.
    pub full_name: String,

    /// ID representing this place. Note that this is represented as a string, not an integer.
    pub id: PlaceId,

    /// Short human-readable representation of the place’s name.
    pub name: String,

    /// The type of location represented by this place.
    pub place_type: String,

    /// URL representing the location of additional place metadata for this place.
    pub url: String,
}

pub type Attributes = HashMap<String, String>;

/// ID of a place.
pub type PlaceId = String;
