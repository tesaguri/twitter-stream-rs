//! Direct messages

use Entities;
use types::DateTime;
use user::{User, UserId};
use util;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DirectMessage {
    #[serde(deserialize_with = "util::deserialize_datetime")]
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

/// ID of a direct message.
pub type DirectMessageId = u64;
