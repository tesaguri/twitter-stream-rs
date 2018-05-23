//! Place

use std::borrow::Cow;
use std::collections::HashMap;

use geometry::Geometry;

/// Represents `place` field in `Tweet`.
///
/// # Reference
///
/// 1. [Places — Twitter Developers][1]
///
/// [1]: https://dev.twitter.com/overview/api/places
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Place<'a> {
    /// Contains a hash of variant information about the place.
    /// See [Place Attributes][1] for more detail.
    ///
    /// [1]: https://dev.twitter.com/overview/api/places#place_attributes
    #[serde(borrow)]
    #[serde(deserialize_with = "::util::deserialize_map_cow_str")]
    pub attributes: Attributes<'a>,

    /// A bounding box of coordinates which encloses this place.
    pub bounding_box: Geometry,

    /// Name of the country containing this place.
    #[serde(borrow)]
    pub country: Cow<'a, str>,

    /// Shortened country code representing the country containing this place.
    #[serde(borrow)]
    pub country_code: Cow<'a, str>,

    /// Full human-readable representation of the place’s name.
    #[serde(borrow)]
    pub full_name: Cow<'a, str>,

    /// ID representing this place. Note that this is represented as a string,
    /// not an integer.
    #[serde(borrow)]
    pub id: PlaceId<'a>,

    /// Short human-readable representation of the place’s name.
    #[serde(borrow)]
    pub name: Cow<'a, str>,

    /// The type of location represented by this place.
    #[serde(borrow)]
    pub place_type: Cow<'a, str>,

    /// URL representing the location of additional place metadata
    /// for this place.
    #[serde(borrow)]
    pub url: Cow<'a, str>,
}

pub type Attributes<'a> = HashMap<Cow<'a, str>, Cow<'a, str>>;

/// ID of a place.
pub type PlaceId<'a> = Cow<'a, str>;
