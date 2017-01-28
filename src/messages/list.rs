//! Lists

pub use serde_types::list::*;

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
