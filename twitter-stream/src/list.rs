//! Lists

use User;
use types::DateTime;
use util;

/// Represents a List.
///
/// # Reference
///
/// 1. [GET lists/show — Twitter Developers](https://dev.twitter.com/rest/reference/get/lists/show)
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct List {
    pub slug: String,
    pub name: String,
    #[serde(deserialize_with = "util::deserialize_datetime")]
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
    /// Represents `mode` field of `List`.
    #[derive(Clone, Debug)]
    pub enum Mode {
        :Public("public"),
        :Private("private");
        :Custom(_),
    }
}

/// ID of a list.
pub type ListId = u64;
