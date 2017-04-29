//! Entities

use tweet::StatusId;
use user::UserId;

pub type MediaId = u64;

/// Represents Entities.
///
/// # Reference
///
/// 1. [Entities — Twitter Developers](https://dev.twitter.com/overview/api/entities)
/// 1. [Entities in Objects — Twitter Developers](https://dev.twitter.com/overview/api/entities-in-twitter-objects)
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Entities {
    /// Represents hashtags which have been parsed out of the Tweet text.
    pub hashtags: Vec<Hashtag>,

    /// Represents media elements uploaded with the Tweet.
    pub media: Option<Vec<Media>>,

    /// Represents URLs included in the `text` of a `Tweet` or within textual fields of a `User` object.
    pub urls: Vec<Url>,

    /// Represents other Twitter users mentioned in the `text` of the `Tweet`.
    pub user_mentions: Vec<UserMention>,

    /// Represents financial symbols which have been parsed out of the Tweet text.
    pub symbols: Vec<Symbol>,
}

/// Represents a hashtag in `hashtags` field of `Entities`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Hashtag {
    /// A pair of integers indicating the offsets within the Tweet text where the hashtag begins and ends.
    /// The first integer represents the location of the `#` character in the Tweet text string.
    /// The second integer represents the location of the first character after the hashtag. Therefore
    /// the difference between the two numbers will be the length of the hashtag name plus one (for the `#` character).
    pub indices: (u64, u64),

    /// Name of the hashtag, minus the leading `#` character.
    pub text: String,
}

/// Represents `media` field in `Entities`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Media {
    /// URL of the media to display to clients.
    pub display_url: String,

    /// An expanded version of `display_url`. Links to the media display page.
    pub expanded_url: String,

    /// ID of the media expressed as a 64-bit integer.
    pub id: MediaId,

    // pub id_str: String,

    /// A pair of integers indicating the offsets within the Tweet text where the URL begins and ends.
    /// The first integer represents the location of the first character of the URL in the Tweet text.
    /// The second integer represents the location of the first non-URL character occurring after the URL
    /// (or the end of the string if the URL is the last part of the Tweet text).
    pub indices: (u64, u64),

    /// An http:// URL pointing directly to the uploaded media file.
    pub media_url: String,

    /// An https:// URL pointing directly to the uploaded media file, for embedding on https pages.
    ///
    /// For media in direct messages, `media_url_https` must be accessed via an authenticated twitter.com session
    /// or by signing a request with the user’s access token using OAuth 1.0A.
    /// It is not possible to directly embed these images in a web page.
    pub media_url_https: String,

    /// An object showing available sizes for the media file.
    pub sizes: Sizes,

    /// For Tweets containing media that was originally associated with a different tweet,
    /// this ID points to the original Tweet.
    pub source_status_id: Option<StatusId>,

    // source_status_id_str: String,

    /// Type of uploaded media.
    #[serde(rename="type")]
    pub kind: String,

    /// Wrapped URL for the media link. This corresponds with the URL embedded directly into the raw Tweet text,
    /// and the values for the `indices` parameter.
    pub url: String,
}

/// Represents the `sizes` field in `Media`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Sizes {
    pub thumb: Size,
    pub large: Size,
    pub medium: Size,
    pub small: Size,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Size {
    /// Height in pixels of this size.
    pub h: u64,

    /// Resizing method used to obtain this size.
    pub resize: Resize,

    /// Width in pixels of this size.
    pub w: u64,
}

string_enums! {
    /// Represents the `resize` field in `Size`.
    #[derive(Clone, Debug)]
    pub enum Resize {
        /// The media was resized to fit one dimension, keeping its native aspect ratio.
        :Fit("fit"),
        /// The media was cropped in order to fit a specific resolution.
        :Crop("crop");
        :Custom(_),
    }
}

/// Represents a URL in `urls` field of `Entities`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Url {
    /// Version of the URL to display to clients.
    #[serde(deserialize_with = "::util::deserialize_default")] // nullable in Retweets.
    #[serde(default)]
    pub display_url: String,

    /// Expanded version of `display_url`.
    #[serde(deserialize_with = "::util::deserialize_default")] // nullable in Retweets.
    #[serde(default)]
    pub expanded_url: String,

    /// A pair of integers representing offsets within the Tweet text where the URL begins and ends.
    /// The first integer represents the location of the first character of the URL in the Tweet text.
    /// The second integer represents the location of the first non-URL character after the end of the URL.
    pub indices: (u64, u64),

    /// Wrapped URL, corresponding to the value embedded directly into the raw Tweet text, and the
    /// values for the `indices` parameter.
    pub url: String,
}

/// Represents a user in `user_mentions` field of `Entities`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct UserMention {
    /// ID of the mentioned user, as an integer.
    pub id: UserId,

    // pub id_str: String,

    /// A pair of integers representing the offsets within the Tweet text where the user reference begins and ends.
    /// The first integer represents the location of the ‘@’ character of the user mention.
    /// The second integer represents the location of the first non-screenname character following the user mention.
    pub indices: (u64, u64),

    /// Display name of the referenced user.
    pub name: String,

    /// Screen name of the referenced user.
    pub screen_name: String,
}

/// Represents a financial symbol in `symbols` field of `Entities`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
pub struct Symbol {
    pub text: String,
    pub indices: (u64, u64),
}
