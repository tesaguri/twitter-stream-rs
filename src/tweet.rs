//! Tweets

use {Entities, Geometry, Place};
use std::collections::HashMap;
use types::{DateTime, FilterLevel, JsonValue, WithheldScope};
use user::{User, UserId};
use util;

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
    #[serde(deserialize_with = "util::deserialize_datetime")]
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
    pub scopes: Option<HashMap<String, JsonValue>>,

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

/// ID of a Tweet.
pub type StatusId = u64;
