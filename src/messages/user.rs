use super::{DateTime, Entities, Tweet};

// https://dev.twitter.com/overview/api/users
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct User {
    contributors_enabled: bool,
    #[serde(deserialize_with = "super::deserialize_datetime")]
    created_at: DateTime,
    default_profile: bool,
    default_profile_image: bool,
    description: Option<String>,
    entities: Option<Entities>,
    favourites_count: u64,
    follow_request_sent: Option<bool>,
    // following: Option<bool>, // deprecated
    followers_count: u64,
    friends_count: u64,
    geo_enabled: bool,
    id: UserId,
    is_translator: bool,
    lang: String,
    listed_count: u64,
    location: Option<String>,
    name: Option<String>,
    // notifications: Option<bool>, // deprecated
    profile_background_image_url: String,
    profile_background_image_url_https: String,
    profile_background_tile: bool,
    profile_banner_url: Option<String>,
    profile_image_url: String,
    profile_image_url_https: String,
    profile_link_color: String,
    profile_sidebar_border_color: String,
    profile_sidebar_fill_color: String,
    profile_text_color: String,
    profile_use_background_image: bool,
    protected: bool,
    screen_name: String,
    // show_all_inline_media: bool, // removed
    status: Option<Box<Tweet>>,
    statuses_count: u64,
    time_zone: Option<String>,
    url: Option<String>,
    utc_offset: Option<i64>,
    verified: bool,
    withheld_in_countries: Option<String>,
    withheld_scope: Option<String>,
}

pub type UserId = u64;
