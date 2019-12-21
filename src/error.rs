//! Error type

use std::error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::str::Utf8Error;

use crate::types::StatusCode;

/// An error occurred while trying to connect to a Stream.
#[derive(Debug)]
pub enum Error<E = Box<dyn error::Error + Send + Sync>> {
    /// An error occured while decoding gzip stream from the server.
    Gzip(io::Error),
    /// An HTTP error from the Stream.
    Http(StatusCode),
    /// Error from the underlying HTTP client while receiving an HTTP response or reading the body.
    Service(E),
    /// Twitter returned a non-UTF-8 string.
    Utf8(Utf8Error),
    /// User-defined error.
    Custom(Box<dyn error::Error + Send + Sync>),
}

impl<E> Error<E> {
    pub fn custom<C>(error: C) -> Self
    where
        C: Into<Box<dyn error::Error + Send + Sync>>,
    {
        Error::Custom(error.into())
    }
}

impl<E: error::Error + 'static> error::Error for Error<E> {
    fn description(&self) -> &str {
        use crate::Error::*;

        match *self {
            Gzip(ref e) => e.description(),
            Http(ref status) => status.canonical_reason().unwrap_or("<unknown status code>"),
            Service(ref e) => e.description(),
            Utf8(ref e) => e.description(),
            Custom(ref e) => e.description(),
        }
    }

    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use crate::Error::*;

        match *self {
            Gzip(ref e) => Some(e),
            Http(_) => None,
            Service(ref e) => Some(e),
            Utf8(ref e) => Some(e),
            Custom(ref e) => Some(&**e),
        }
    }
}

impl<E: Display> Display for Error<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use crate::Error::*;

        match *self {
            Gzip(ref e) => Display::fmt(e, f),
            Http(ref code) => Display::fmt(code, f),
            Service(ref e) => Display::fmt(e, f),
            Utf8(ref e) => Display::fmt(e, f),
            Custom(ref e) => Display::fmt(e, f),
        }
    }
}
