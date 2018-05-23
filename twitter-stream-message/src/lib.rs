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
pub use json::Result;
pub use list::List;
pub use message::StreamMessage;
pub use place::Place;
pub use tweet::Tweet;
pub use user::User;

/// Alias to [`StreamMessage::from_str`][1].
/// Parses a JSON string returned from Twitter Streaming API.
///
/// [1]: message/enum.StreamMessage.html#method.from_str
#[inline]
pub fn from_str<'a>(json: &'a str) -> Result<StreamMessage<'a>> {
    StreamMessage::from_str(json)
}
