//! Error types

pub use default_client::Error as TlsError;
pub use hyper::Error as HyperError;
pub use json::Error as JsonError;

use message::Disconnect;
use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::result;
use std::io;
use types::StatusCode;

/// An error occurred while trying to connect to a Stream.
#[allow(unreachable_patterns)]
#[derive(Debug)]
pub enum Error {
    /// An error occured while parsing the gzip header of the response from the server.
    Gzip(io::Error),
    /// An HTTP error from the Stream.
    Http(StatusCode),
    /// An error from the `hyper` crate.
    Hyper(HyperError),
    /// An error occured when initializing a TLS client.
    Tls(TlsError),
}

/// An error occured while listening on a Stream.
#[derive(Debug)]
pub enum StreamError {
    /// The Stream has been disconnected by the server.
    Disconnect(Disconnect),
    /// An I/O error.
    Io(io::Error),
    /// Failed to parse a JSON message from a Stream.
    Json(JsonError),
}

pub type Result<T> = result::Result<T, Error>;

impl StdError for Error {
    fn description(&self) -> &str {
        use Error::*;

        #[allow(unreachable_patterns)]
        match *self {
            Gzip(ref e) => e.description(),
            Http(ref status) => status.canonical_reason().unwrap_or("<unknown status code>"),
            Hyper(ref e) => e.description(),
            Tls(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use Error::*;

        #[allow(unreachable_patterns)]
        match *self {
            Gzip(ref e) => Some(e),
            Http(_) => None,
            Hyper(ref e) => Some(e),
            Tls(ref e) => Some(e),
        }
    }
}

impl StdError for StreamError {
    fn description(&self) -> &str {
        use StreamError::*;

        match *self {
            Disconnect(ref d) => &d.reason,
            Io(ref e) => e.description(),
            Json(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use StreamError::*;

        match *self {
            Disconnect(_) => None,
            Io(ref e) => Some(e),
            Json(ref e) => Some(e),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Error::*;

        #[allow(unreachable_patterns)]
        match *self {
            Gzip(ref e) => Display::fmt(e, f),
            Http(ref code) => Display::fmt(code, f),
            Hyper(ref e) => Display::fmt(e, f),
            Tls(ref e) => Display::fmt(e, f),
        }
    }
}

impl Display for StreamError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use StreamError::*;

        match *self {
            Disconnect(ref d) => Display::fmt(d, f),
            Io(ref e) => Display::fmt(e, f),
            Json(ref e) => Display::fmt(e, f),
        }
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

// Assuming that `TlsError` is not `JsonError`, `StatusCode`, `UrlError`, `HyperError` or `io::Error`.
impl From<TlsError> for Error {
    fn from(e: TlsError) -> Self {
        Error::Tls(e)
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
