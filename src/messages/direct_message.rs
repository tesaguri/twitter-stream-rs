use super::{DateTime, Entities, User, UserId};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DirectMessage {
    #[serde(deserialize_with = "super::deserialize_datetime")]
    created_at: DateTime,
    entities: Entities,
    id: u64,
    recipient: User,
    recipient_id: UserId,
    recipient_screen_name: String,
    sender: User,
    sender_id: UserId,
    sender_screen_name: String,
    text: String,
}
