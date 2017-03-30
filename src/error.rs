//! Error types

pub use hyper::Error as HyperError;
pub use json::Error as JsonError;

use message::Disconnect;
use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::str::Utf8Error;
use types::StatusCode;

/// An error occurred while trying to connect to a Stream.
#[derive(Debug)]
pub enum Error {
    /// An HTTP error from the Stream.
    Http(StatusCode),
    /// An error from the `hyper` crate.
    Hyper(HyperError),
}

/// An error occured while listening on a Stream.
#[derive(Debug)]
pub enum StreamError {
    /// The Stream has been disconnected by the server.
    Disconnect(Disconnect),
    Hyper(HyperError),
    /// An I/O error.
    Io(io::Error),
    /// Failed to parse a JSON message from a Stream.
    Json(JsonError),
    Utf8(Utf8Error),
}

pub enum JsonStreamError {
    Hyper(HyperError),
    Utf8(Utf8Error),
}

impl StdError for Error {
    fn description(&self) -> &str {
        use Error::*;

        match *self {
            Http(ref status) => status.canonical_reason().unwrap_or("<unknown status code>"),
            Hyper(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use Error::*;

        match *self {
            Http(_) => None,
            Hyper(ref e) => Some(e),
        }
    }
}

impl StdError for StreamError {
    fn description(&self) -> &str {
        use StreamError::*;

        match *self {
            Disconnect(ref d) => &d.reason,
            Hyper(ref e) => e.description(),
            Io(ref e) => e.description(),
            Json(ref e) => e.description(),
            Utf8(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use StreamError::*;

        match *self {
            Disconnect(_) => None,
            Hyper(ref e) => Some(e),
            Io(ref e) => Some(e),
            Json(ref e) => Some(e),
            Utf8(ref e) => Some(e),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Error::*;

        match *self {
            Http(ref code) => Display::fmt(code, f),
            Hyper(ref e) => Display::fmt(e, f),
        }
    }
}

impl Display for StreamError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use StreamError::*;

        match *self {
            Disconnect(ref d) => Display::fmt(d, f),
            Hyper(ref e) => Display::fmt(e, f),
            Io(ref e) => Display::fmt(e, f),
            Json(ref e) => Display::fmt(e, f),
            Utf8(ref e) => Display::fmt(e, f),
        }
    }
}

impl From<JsonStreamError> for StreamError {
    fn from(e: JsonStreamError) -> Self {
        match e {
            JsonStreamError::Hyper(e) => StreamError::Hyper(e),
            JsonStreamError::Utf8(e) => StreamError::Utf8(e),
        }
    }
}

impl From<HyperError> for JsonStreamError {
    fn from(e: HyperError) -> Self {
        JsonStreamError::Hyper(e)
    }
}

impl From<Utf8Error> for JsonStreamError {
    fn from(e: Utf8Error) -> Self {
        JsonStreamError::Utf8(e)
    }
}

impl From<JsonError> for StreamError {
    fn from(e: JsonError) -> Self {
        StreamError::Json(e)
    }
}

impl From<StatusCode> for Error {
    fn from(e: StatusCode) -> Self {
        Error::Http(e)
    }
}

impl From<HyperError> for Error {
    fn from(e: HyperError) -> Self {
        Error::Hyper(e)
    }
}

impl From<io::Error> for StreamError {
    fn from(e: io::Error) -> Self {
        StreamError::Io(e)
    }
}
