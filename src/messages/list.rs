use super::{DateTime, User};

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

string_enums! {
    #[derive(Clone, Debug)]
    pub enum Mode {
        Public("public"),
        Private("private");
        Unknown(_),
    }
}

pub type ListId = u64;
