//! Common types used across the crate.

pub use hyper::Method as RequestMethod;
pub use hyper::StatusCode;
pub use url::Url;

use bytes::Bytes;
use std::fmt::{self, Display, Formatter};
use std::ops::Deref;
use std::str::{self, Utf8Error};

string_enums! {
    /// Represents the `filter_level` parameter in API requests.
    #[derive(Clone, Debug)]
    pub enum FilterLevel {
        :None("none"),
        :Low("low"),
        :Medium("medium");
        :Custom(_),
    }

    /// A value for `with` parameter for User and Site Streams.
    #[derive(Clone, Debug)]
    pub enum With {
        /// Instruct the stream to send messages only from the user associated with that stream.
        /// The default for Site Streams.
        :User("user"),
        /// Instruct the stream to send messages from accounts the user follows as well, equivalent
        /// to the user’s home timeline. The default for User Streams.
        :Following("following");
        /// Custom value.
        :Custom(_),
    }
}

/// A string type returned by `TwitterStream`.
// It is basically a wrapper type over `Bytes` that gurantees the contained bytes are valid UTF8 string.
// Inspired by hyper::http::str::ByteStr.
#[derive(Clone, PartialEq, PartialOrd, Ord, Eq, Debug, Hash)]
pub struct JsonStr {
    inner: Bytes,
}

impl JsonStr {
    #[doc(hidden)]
    pub fn from_utf8(v: Bytes) -> Result<Self, Utf8Error> {
        str::from_utf8(&v)?;
        unsafe { Ok(JsonStr::from_utf8_unchecked(v)) }
    }

    pub fn from_static(s: &'static str) -> Self {
        JsonStr {
            inner: Bytes::from_static(s.as_ref()),
        }
    }

    #[doc(hidden)]
    pub unsafe fn from_utf8_unchecked(v: Bytes) -> Self {
        JsonStr {
            inner: v,
        }
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.inner) }
    }
}

impl AsRef<str> for JsonStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for JsonStr {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl Display for JsonStr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl From<String> for JsonStr {
    fn from(s: String) -> Self {
        JsonStr {
            inner: s.into_bytes().into(),
        }
    }
}

impl<'a> From<&'a str> for JsonStr {
    fn from(s: &'a str) -> Self {
        JsonStr {
            inner: s.into(),
        }
    }
}

impl From<JsonStr> for Bytes {
    fn from(s: JsonStr) -> Self {
        s.inner
    }
}

impl ::std::default::Default for FilterLevel {
    fn default() -> Self {
        FilterLevel::None
    }
}
