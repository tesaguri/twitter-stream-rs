/*!
# Twitter Stream Message

A library for parsing JSON messages returned by Twitter Streaming API.
*/

extern crate chrono;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json as json;

#[macro_use]
mod util;

pub mod direct_message;
pub mod entities;
pub mod geometry;
pub mod list;
pub mod message;
pub mod place;
pub mod tweet;
pub mod types;
pub mod user;

pub use direct_message::DirectMessage;
pub use entities::Entities;
pub use geometry::Geometry;
pub use json::Error;
pub use list::List;
pub use message::StreamMessage;
pub use place::Place;
pub use tweet::Tweet;
pub use user::User;

/// Parse a JSON string returned from Twitter Streaming API.
///
/// ```
/// use twitter_stream_message::message::{Delete, StreamMessage};
///
/// let parsed = twitter_stream_message::parse(r#"{
///     "delete":{
///         "status":{
///             "id":1234,
///             "id_str":"1234",
///             "user_id":3,
///             "user_id_str":"3"
///         }
///     }
/// }"#).unwrap();
/// let expected = StreamMessage::Delete(Delete {
///     id: 1234,
///     user_id: 3,
/// });
///
/// assert_eq!(parsed, expected);
pub fn parse<'a>(json: &'a str) -> Result<StreamMessage<'a>, Error> {
    json::from_str(json)
}
