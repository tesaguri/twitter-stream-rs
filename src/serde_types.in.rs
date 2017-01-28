pub use messages::{DateTime, Entities, FilterLevel, Geometry, Place, StatusId, Tweet, User, UserId, WithheldScope};
use serde::{Deserialize, Deserializer, Error};

pub mod direct_message {
    use messages::direct_message::*;
    use super::*;

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct DirectMessage {
        #[serde(deserialize_with = "super::deserialize_datetime")]
        pub created_at: DateTime,
        pub entities: Entities,
        pub id: DirectMessageId,
        pub recipient: User,
        pub recipient_id: UserId,
        pub recipient_screen_name: String,
        pub sender: User,
        pub sender_id: UserId,
        pub sender_screen_name: String,
        pub text: String,
    }
}

pub mod list {
    use messages::list::*;
    use super::*;

    /// Represents a List.
    ///
    /// # Reference
    ///
    /// 1. [GET lists/show — Twitter Developers](https://dev.twitter.com/rest/reference/get/lists/show)
    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct List {
        pub slug: String,
        pub name: String,
        #[serde(deserialize_with = "super::deserialize_datetime")]
        pub created_at: DateTime,
        pub uri: String,
        pub subscriber_count: u64,
        pub member_count: u64,
        pub mode: Mode,
        pub id: ListId,
        pub full_name: String,
        pub description: String,
        pub user: User,
        pub following: bool,
    }
}

pub mod stream {
    use messages::stream::*;
    use super::*;

    /// Represents a range of Tweets whose geolocated data must be stripped.
    #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
    pub struct ScrubGeo {
        pub user_id: UserId,
        pub up_to_status_id: StatusId,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
    pub struct Limit {
        pub track: u64,
    }

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
    pub struct StatusWithheld {
        pub id: StatusId,
        pub user_id: UserId,
        pub withheld_in_countries: Vec<String>,
    }

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
    pub struct UserWithheld {
        pub id: UserId,
        pub withheld_in_countries: Vec<String>,
    }

    /// Indicates why a stream was closed.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
    pub struct Disconnect {
        pub code: DisconnectCode,
        pub stream_name: String,
        pub reason: String,
    }

    /// Represents a control message.
    #[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
    pub struct Control {
        control_uri: String,
    }
}

pub mod stream_delete {
    use super::*;
    #[allow(dead_code)]
    #[derive(Deserialize)]
    pub struct Status { id: StatusId, user_id: UserId }
}

pub mod entities {
    use messages::entities::*;
    use super::*;
    use serde::de::{Deserializer, Visitor};

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

    /// Represents a URL in `urls` field of `Entities`.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
    pub struct Url {
        /// Version of the URL to display to clients.
        #[serde(deserialize_with = "nullable_string")] // nullable in Retweets.
        #[serde(default)]
        pub display_url: String,

        /// Expanded version of `display_url`.
        #[serde(deserialize_with = "nullable_string")] // nullable in Retweets.
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

    fn nullable_string<D: Deserializer>(d: &mut D) -> Result<String, D::Error> {
        struct NSVisitor;

        impl Visitor for NSVisitor {
            type Value = String;

            fn visit_str<E>(&mut self, v: &str) -> Result<String, E> {
                Ok(v.to_owned())
            }

            fn visit_string<E>(&mut self, v: String) -> Result<String, E> {
                Ok(v)
            }

            fn visit_unit<E>(&mut self) -> Result<String, E> {
                Ok(String::new())
            }
        }

        d.deserialize_string(NSVisitor)
    }
}

pub mod place {
    use messages::place::*;
    use super::*;

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
}

pub mod tweet {
    use super::*;
    use json::Value;
    use std::collections::HashMap;

    /// Represents a Tweet.
    ///
    /// # Reference
    ///
    /// [Tweets — Twitter Developers](https://dev.twitter.com/overview/api/tweets)
    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct Tweet {
        // pub contributors: Option<_>, // deprecated

        /// Represents the geographic location of this Tweet as reported by the user or client application.
        pub coordinates: Option<Geometry>,

        /// UTC time when this Tweet was created.
        #[serde(deserialize_with = "super::deserialize_datetime")]
        pub created_at: DateTime,

        // pub current_user_retweet: Option<StatusId>,

        /// Entities which have been parsed out of the text of the Tweet.
        pub entities: Entities,

        /// Indicates approximately how many times this Tweet has been liked by Twitter users.
        pub favorite_count: Option<u64>,

        /// *Perspectival* Indicates whether this Tweet has been liked by the authenticating user.
        pub favorited: Option<bool>,

        /// Indicates the maximum value of the `filter_level` parameter which may be used and still stream this Tweet.
        /// So a value of `Medium` will be streamed on `None`, `Low`, and `Medium` streams.
        pub filter_level: Option<FilterLevel>,

        // pub geo: _, // deprecated in favor of `coordinates` field.

        /// The integer representation of the unique identifier for this Tweet.
        pub id: StatusId,

        // pub id_str: String,

        /// If the represented Tweet is a reply, this field will contain the screen name of the original Tweet’s author.
        pub in_reply_to_screen_name: Option<String>,

        /// If the represented Tweet is a reply, this field will contain the integer representation of
        /// the original Tweet’s ID.
        pub in_reply_to_status_id: Option<StatusId>,

        // pub in_reply_to_status_id_str: Option<String>,

        /// If the represented Tweet is a reply, this field will contain the integer representation of the original Tweet’s
        /// author ID. This will not necessarily always be the user directly mentioned in the Tweet.
        pub in_reply_to_user_id: Option<UserId>,

        // pub in_reply_to_user_id_str: Option<String>,

        pub is_quote_status: bool,

        /// When present, indicates a [BCP 47][1] language identifier corresponding to the machine-detected language of
        /// the Tweet text, or `und` if no language could be detected.
        /// [1]: http://tools.ietf.org/html/bcp47
        pub lang: Option<String>,

        /// When present, indicates that the tweet is associated (but not necessarily originating from) a [Place][1].
        /// [1]: struct.Place.html
        pub place: Option<Place>,

        /// This field only surfaces when a Tweet contains a link. The meaning of the field doesn’t pertain to
        /// the Tweet content itself, but instead it is an indicator that the URL contained in the Tweet may contain
        /// content or media identified as sensitive content.
        pub possibly_sensitive: Option<bool>,

        /// This field only surfaces when the Tweet is a quote Tweet. This field contains the integer value Tweet ID of the
        /// quoted Tweet.
        pub quoted_status_id: Option<StatusId>,

        // pub quoted_status_id_str: Option<String>,

        /// This field only surfaces when the Tweet is a quote Tweet. This attribute contains the `Tweet` object of
        /// the original Tweet that was quoted.
        pub quoted_status: Option<Box<Tweet>>,

        /// A set of key-value pairs indicating the intended contextual delivery of the containing Tweet.
        /// Currently used by Twitter’s Promoted Products.
        pub scopes: Option<HashMap<String, Value>>,

        /// Number of times this Tweet has been retweeted.
        pub retweet_count: u64,

        /// *Perspectival* Indicates whether this Tweet has been retweeted by the authenticating user.
        pub retweeted: bool,

        /// Users can amplify the broadcast of Tweets authored by other users by [retweeting][1].
        /// Retweets can be distinguished from typical Tweets by the existence of a `retweeted_status` attribute.
        /// This attribute contains a representation of the original Tweet that was retweeted.
        /// [1]: https://dev.twitter.com/rest/reference/post/statuses/retweet/%3Aid
        ///
        /// Note that retweets of retweets do not show representations of the intermediary retweet,
        /// but only the original Tweet. (Users can also [unretweet][2] a retweet they created by deleting their retweet.)
        /// [2]: https://dev.twitter.com/rest/reference/post/statuses/destroy/%3Aid
        pub retweeted_status: Option<Box<Tweet>>,

        /// Utility used to post the Tweet, as an HTML-formatted string.
        /// Tweets from the Twitter website have a source value of `web`.
        pub source: String,

        /// The actual UTF-8 text of the status update.
        /// See [twitter-text][1] for details on what is currently considered valid characters.
        /// [1]: https://github.com/twitter/twitter-text/blob/master/rb/lib/twitter-text/regex.rb
        pub text: String,

        /// Indicates whether the value of the `text` parameter was truncated, for example, as a result of a retweet
        /// exceeding the 140 character Tweet length. Truncated text will end in ellipsis, like this `...`
        ///
        /// Since Twitter now rejects long Tweets vs truncating them, the large majority of Tweets will have this set to
        /// `false`.
        ///
        /// Note that while native retweets may have their toplevel `text` property shortened, the original text will be
        /// available under the `retweeted_status` object and the `truncated` parameter will be set to the value of
        /// the original status (in most cases, `false`).
        pub truncated: bool,

        /// The user who posted this Tweet. Perspectival attributes embedded within this object are unreliable.
        pub user: User,

        /// When set to `true`, it indicates that this piece of content has been withheld due to a [DMCA complaint][1].
        /// [1]: http://en.wikipedia.org/wiki/Digital_Millennium_Copyright_Act
        #[serde(default)]
        pub withheld_copyright: bool,

        /// When present, indicates a list of uppercase [two-letter country codes][1] this content is withheld from.
        /// Twitter supports the following non-country values for this field:
        /// [1]: http://en.wikipedia.org/wiki/ISO_3166-1_alpha-2
        ///
        /// - `XX` - Content is withheld in all countries
        /// - `XY` - Content is withheld due to a DMCA request.
        #[serde(default)]
        pub withheld_in_countries: Vec<String>,

        /// When present, indicates whether the content being withheld is the `Status` or a `User`.
        pub withheld_scope: Option<WithheldScope>,
    }
}

pub mod user {
    use super::*;

    /// Represents a user on Twitter.
    ///
    /// # Reference
    ///
    /// 1. [Users — Twitter Developers](https://dev.twitter.com/overview/api/users)
    #[derive(Clone, Debug, Deserialize, PartialEq)]
    pub struct User {
        /// Indicates that the user has an account with “contributor mode” enabled,
        /// allowing for Tweets issued by the user to be co-authored by another account. Rarely `true`.
        pub contributors_enabled: bool,

        /// The UTC datetime that the user account was created on Twitter.
        #[serde(deserialize_with = "super::deserialize_datetime")]
        pub created_at: DateTime,

        /// When `true`, indicates that the user has not altered the theme or background of their user profile.
        pub default_profile: bool,

        /// When `true`, indicates that the user has not uploaded their own avatar and a default egg avatar is used instead.
        pub default_profile_image: bool,

        /// The user-defined UTF-8 string describing their account.
        pub description: Option<String>,

        // pub entities: Entities, // does not appear in stream messages

        /// The number of tweets this user has favorited in the account’s lifetime.
        /// British spelling used in the field name for historical reasons.
        pub favourites_count: u64,

        /// *Perspectival*. When `true`, indicates that the authenticating user has issued a follow request to
        /// this protected user account.
        pub follow_request_sent: Option<bool>,

        // pub following: Option<bool>, // deprecated

        /// The number of followers this account currently has. Under certain conditions of duress,
        /// this field will temporarily indicate `0`.
        pub followers_count: u64,

        /// The number of users this account is following (AKA their “followings”). Under certain conditions of duress,
        /// this field will temporarily indicate `0`.
        pub friends_count: u64,

        /// When `true`, indicates that the user has enabled the possibility of geotagging their Tweets.
        /// This field must be `true` for the current user to attach geographic data when using [POST statuses / update][1].
        /// [1]: https://dev.twitter.com/rest/reference/post/statuses/update
        pub geo_enabled: bool,

        // pub has_extended_profile: Option<bool>, // does not appear in stream message

        /// The integer representation of the unique identifier for this User.
        pub id: UserId,

        // pub id_str: String,

        /// When `true`, indicates that the user is a participant in Twitter’s [translator community][1].
        /// [1]: http://translate.twttr.com/
        pub is_translator: bool,

        /// The [BCP 47][1] code for the user’s self-declared user interface language. May or may not have
        /// anything to do with the content of their Tweets.
        /// [1]: http://tools.ietf.org/html/bcp47
        pub lang: String,

        /// The number of public lists that this user is a member of.
        pub listed_count: u64,

        /// The user-defined location for this account’s profile. Not necessarily a location nor parseable.
        /// This field will occasionally be fuzzily interpreted by the Search service.
        pub location: Option<String>,

        /// The name of the user, as they’ve defined it. Not necessarily a person’s name.
        /// Typically capped at 20 characters, but subject to change.
        pub name: String,

        // pub notifications: Option<bool>, // deprecated

        /// The hexadecimal color chosen by the user for their background.
        pub profile_background_color: String,

        /// A HTTP-based URL pointing to the background image the user has uploaded for their profile.
        pub profile_background_image_url: String,

        /// A HTTPS-based URL pointing to the background image the user has uploaded for their profile.
        pub profile_background_image_url_https: String,

        /// When `true`, indicates that the user’s `profile_background_image_url` should be tiled when displayed.
        pub profile_background_tile: bool,

        /// The HTTPS-based URL pointing to the standard web representation of the user’s uploaded profile banner.
        /// By adding a final path element of the URL, you can obtain different image sizes optimized for specific displays.
        ///
        /// In the future, an API method will be provided to serve these URLs so that you need not modify the original URL.
        ///
        /// For size variations, please see [User Profile Images and Banners][1].
        /// [1]: https://dev.twitter.com/basics/user-profile-images-and-banners
        pub profile_banner_url: Option<String>,

        /// A HTTP-based URL pointing to the user’s avatar image. See [User Profile Images and Banners][1].
        /// [1]: https://dev.twitter.com/basics/user-profile-images-and-banners
        pub profile_image_url: String,

        /// A HTTPS-based URL pointing to the user’s avatar image.
        pub profile_image_url_https: String,

        /// The hexadecimal color the user has chosen to display links with in their Twitter UI.
        pub profile_link_color: String,

        // pub profile_location: Option<_>, // does not appear in stream message

        /// The hexadecimal color the user has chosen to display sidebar borders with in their Twitter UI.
        pub profile_sidebar_border_color: String,

        /// The hexadecimal color the user has chosen to display sidebar backgrounds with in their Twitter UI.
        pub profile_sidebar_fill_color: String,

        /// The hexadecimal color the user has chosen to display text with in their Twitter UI.
        pub profile_text_color: String,

        /// When `true`, indicates the user wants their uploaded background image to be used.
        pub profile_use_background_image: bool,

        /// When `true`, indicates that this user has chosen to protect their Tweets.
        /// See [About Public and Protected Tweets][1].
        ///
        /// [1]: https://support.twitter.com/articles/14016-about-public-and-protected-tweets
        pub protected: bool,

        /// The screen name, handle, or alias that this user identifies themselves with.
        ///
        /// `screen_name`s are unique but subject to change. Use `id` as a user identifier whenever possible.
        ///
        /// Typically a maximum of 15 characters long, but some historical accounts may exist with longer names.
        pub screen_name: String,

        // pub show_all_inline_media: bool, // removed

        // pub status: Option<Box<Tweet>>, // does not appear in stream messages

        /// The number of tweets (including retweets) issued by the user.
        pub statuses_count: u64,

        /// A string describing the Time Zone this user declares themselves within.
        pub time_zone: Option<String>,

        /// A URL provided by the user in association with their profile.
        pub url: Option<String>,

        /// The offset from GMT/UTC in seconds.
        pub utc_offset: Option<i64>,

        /// When `true`, indicates that the user has a verified account. See [Verified Accounts][1].
        /// [1]: https://support.twitter.com/articles/119135-faqs-about-verified-accounts
        pub verified: bool,

        /// When present, indicates a textual representation of the two-letter country codes this user is withheld from.
        pub withheld_in_countries: Option<String>,

        /// When present, indicates whether the content being withheld is the `Status` or a `User`.
        pub withheld_scope: Option<WithheldScope>,
    }
}

pub mod geometry {
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct Position(
        /// Longitude
        pub f64,
        /// Latitude
        pub f64,
        // Twitter API does not provide altitudes
    );
}

fn parse_datetime(s: &str) -> ::chrono::format::ParseResult<DateTime> {
    use chrono::{TimeZone, UTC};
    UTC.datetime_from_str(s, "%a %b %e %H:%M:%S %z %Y")
}

fn deserialize_datetime<D: Deserializer>(d: &mut D) -> Result<DateTime, D::Error> {
    parse_datetime(&String::deserialize(d)?).map_err(|e| D::Error::custom(e.to_string()))
}
