use std::collections::HashMap;
use super::Geometry;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Place {
    pub attributes: Attributes,
    pub bounding_box: Geometry,
    pub country: String,
    pub country_code: String,
    pub full_name: String,
    pub id: String,
    pub name: String,
    pub place_type: String,
    pub url: String,
}

pub type Attributes = HashMap<String, String>;
