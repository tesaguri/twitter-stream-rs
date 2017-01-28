//! Place

use std::collections::HashMap;

pub use serde_types::place::*;

pub type Attributes = HashMap<String, String>;

/// ID of a place.
pub type PlaceId = String;
