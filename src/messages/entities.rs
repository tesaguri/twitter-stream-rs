// https://dev.twitter.com/overview/api/entities

use super::{StatusId, UserId};

pub type MediaId = u64;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Entities {
    pub hashtags: Vec<Hashtag>,
    pub media: Option<Vec<Media>>,
    pub urls: Vec<Url>,
    pub user_mentions: Vec<UserMention>,
    pub symbols: Vec<Symbol>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct ExtendedEntities {
    pub id: MediaId,
    pub media_url: String,
    pub media_url_https: String,
    pub url: String,
    pub display_url: String,
    pub expanded_url: String,
    pub sizes: Sizes,
    #[serde(rename = "type")]
    pub kind: MediaKind,
    pub indices: (u64, u64),
    pub video_info: Option<VideoInfo>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Hashtag {
    pub indices: (u64, u64),
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Media {
    pub display_url: String,
    pub expanded_url: String,
    pub id: MediaId,
    pub indices: (u64, u64),
    pub media_url: String,
    pub media_url_https: String,
    pub sizes: Sizes,
    pub source_status_id: Option<StatusId>,
    #[serde(rename="type")]
    pub kind: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Size {
    pub h: u64,
    pub resize: Resize,
    pub w: u64,
}

string_enums! {
    #[derive(Clone, Debug)]
    pub enum Resize {
        Fit("fit"),
        Crop("crop");
        Unknown(_),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Sizes {
    pub thumb: Size,
    pub large: Size,
    pub medium: Size,
    pub small: Size,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Url {
    pub display_url: String,
    pub expanded_url: String,
    pub indices: (u64, u64),
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct UserMention {
    pub id: UserId,
    pub indices: (u64, u64),
    pub name: String,
    pub screen_name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Symbol {
    pub text: String,
    pub indices: (u64, u64),
}

string_enums! {
    #[derive(Clone, Debug)]
    pub enum MediaKind {
        Photo("photo"),
        AnimatedGif("animated_gif"),
        Video("video");
        Unknown(_),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct VideoInfo {
    pub aspect_ratio: (u64, u64),
    pub duration_millis: u64,
    pub variants: Option<Vec<Variant>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Variant {
    pub bitrate: u64,
    pub content_type: String,
    pub url: String,
}
