//! Direct messages

use std::borrow::Cow;

use Entities;
use types::DateTime;
use user::{User, UserId};
use util;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DirectMessage<'a> {
    #[serde(deserialize_with = "util::deserialize_datetime")]
    pub created_at: DateTime,

    #[serde(borrow)]
    pub entities: Entities<'a>,

    pub id: DirectMessageId,

    #[serde(borrow)]
    pub recipient: User<'a>,

    pub recipient_id: UserId,

    #[serde(borrow)]
    pub recipient_screen_name: Cow<'a, str>,

    #[serde(borrow)]
    pub sender: User<'a>,

    pub sender_id: UserId,

    #[serde(borrow)]
    pub sender_screen_name: Cow<'a, str>,

    #[serde(borrow)]
    pub text: Cow<'a, str>,
}

/// ID of a direct message.
pub type DirectMessageId = u64;
