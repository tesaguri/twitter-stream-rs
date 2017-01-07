use super::DateTime;

// https://dev.twitter.com/overview/api/users
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct User {
    pub contributors_enabled: bool,
    #[serde(deserialize_with = "super::deserialize_datetime")]
    pub created_at: DateTime,
    pub default_profile: bool,
    pub default_profile_image: bool,
    pub description: Option<String>,
    // pub entities: Entities, // does not appear in stream messages
    pub favourites_count: u64,
    pub follow_request_sent: Option<bool>,
    // pub following: Option<bool>, // deprecated
    pub followers_count: u64,
    pub friends_count: u64,
    pub geo_enabled: bool,
    // pub has_extended_profile: Option<bool>, // does not appear in stream message
    pub id: UserId,
    pub is_translator: bool,
    pub lang: String,
    pub listed_count: u64,
    pub location: Option<String>,
    pub name: String,
    // pub notifications: Option<bool>, // deprecated
    pub profile_background_color: String,
    pub profile_background_image_url: String,
    pub profile_background_image_url_https: String,
    pub profile_background_tile: bool,
    pub profile_banner_url: Option<String>,
    pub profile_image_url: String,
    pub profile_image_url_https: String,
    pub profile_link_color: String,
    // pub profile_location: Option<_>, // does not appear in stream message
    pub profile_sidebar_border_color: String,
    pub profile_sidebar_fill_color: String,
    pub profile_text_color: String,
    pub profile_use_background_image: bool,
    pub protected: bool,
    pub screen_name: String,
    // pub show_all_inline_media: bool, // removed
    // pub status: Option<Box<Tweet>>, // does not appear in stream messages
    pub statuses_count: u64,
    pub time_zone: Option<String>,
    pub url: Option<String>,
    pub utc_offset: Option<i64>,
    pub verified: bool,
    pub withheld_in_countries: Option<String>,
    pub withheld_scope: Option<String>,
}

pub type UserId = u64;
