//! Error types

pub use default_connector::Error as TlsError;
pub use hyper::Error as HyperError;

use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::str::Utf8Error;
use types::StatusCode;

/// An error occurred while trying to connect to a Stream.
#[derive(Debug)]
pub enum Error {
    /// An HTTP error from the Stream.
    Http(StatusCode),
    /// An error from the `hyper` crate.
    Hyper(HyperError),
    TimedOut,
    Tls(TlsError),
    Utf8(Utf8Error),
    Custom(Box<StdError + Send + Sync>),
}

impl Error {
    pub fn custom<E>(error: E) -> Self where Box<StdError + Send + Sync>: From<E> {
        Error::Custom(From::from(error))
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use Error::*;

        match *self {
            Http(ref status) => status.canonical_reason().unwrap_or("<unknown status code>"),
            Hyper(ref e) => e.description(),
            TimedOut => "timed out",
            Tls(ref e) => e.description(),
            Utf8(ref e) => e.description(),
            Custom(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use Error::*;

        match *self {
            Hyper(ref e) => Some(e),
            Utf8(ref e) => Some(e),
            Custom(ref e) => Some(e.as_ref()),
            Tls(ref e) => Some(e),
            Http(_) | TimedOut => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Error::*;

        match *self {
            Http(ref code) => Display::fmt(code, f),
            Hyper(ref e) => Display::fmt(e, f),
            TimedOut => Display::fmt(self.description(), f),
            Tls(ref e) => Display::fmt(e, f),
            Utf8(ref e) => Display::fmt(e, f),
            Custom(ref e) => Display::fmt(e, f),
        }
    }
}
