//! Lists

use std::borrow::Cow;

use User;
use types::DateTime;
use util;

/// Represents a List.
///
/// # Reference
///
/// 1. [GET lists/show — Twitter Developers][1]
///
/// [1]: https://dev.twitter.com/rest/reference/get/lists/show
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct List<'a> {
    #[serde(borrow)]
    pub slug: Cow<'a, str>,

    #[serde(borrow)]
    pub name: Cow<'a, str>,

    #[serde(deserialize_with = "util::deserialize_datetime")]
    pub created_at: DateTime,

    #[serde(borrow)]
    pub uri: Cow<'a, str>,

    pub subscriber_count: u64,

    pub member_count: u64,

    #[serde(borrow)]
    pub mode: Mode<'a>,

    pub id: ListId,

    #[serde(borrow)]
    pub full_name: Cow<'a, str>,

    #[serde(borrow)]
    pub description: Cow<'a, str>,

    #[serde(borrow)]
    pub user: User<'a>,

    pub following: bool,
}

string_enums! {
    /// Represents `mode` field of `List`.
    #[derive(Clone, Debug)]
    pub enum Mode<'a> {
        Public("public"),
        Private("private");
        Custom(_),
    }
}

/// ID of a list.
pub type ListId = u64;
