use super::{DateTime, User};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct List {
    slug: String,
    name: String,
    #[serde(deserialize_with = "super::deserialize_datetime")]
    created_at: DateTime,
    uri: String,
    subscriber_count: u64,
    member_count: u64,
    mode: Mode,
    id: ListId,
    full_name: String,
    description: String,
    user: User,
    following: bool,
}

string_enums! {
    #[derive(Clone, Debug)]
    pub enum Mode {
        Public("public"),
        Private("private");
        Unknown(_),
    }
}

pub type ListId = u64;
