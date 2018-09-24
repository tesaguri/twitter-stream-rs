//! Error types

pub use default_connector::Error as TlsError;
pub use hyper::Error as HyperError;

use std::error::{self, Error as _Error};
use std::fmt::{self, Display, Formatter};
use std::io;
use std::str::Utf8Error;

use types::StatusCode;

/// An error occurred while trying to connect to a Stream.
#[derive(Debug)]
pub enum Error {
    /// An error occured while decoding gzip stream from the server.
    Gzip(io::Error),
    /// An HTTP error from the Stream.
    Http(StatusCode),
    /// An error from the `hyper` crate.
    Hyper(HyperError),
    /// The stream has timed out.
    TimedOut,
    /// An error occured when initializing a TLS client.
    Tls(TlsError),
    /// Twitter returned a non-UTF-8 string.
    Utf8(Utf8Error),
    /// User-defined error.
    Custom(Box<error::Error + Send + Sync>),
}

impl Error {
    pub fn custom<E>(error: E) -> Self
    where
        E: Into<Box<error::Error + Send + Sync>>,
    {
        Error::Custom(error.into())
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        use Error::*;

        match *self {
            Gzip(ref e) => e.description(),
            Http(ref status) => status.canonical_reason().unwrap_or("<unknown status code>"),
            Hyper(ref e) => e.description(),
            TimedOut => "timed out",
            Tls(ref e) => e.description(),
            Utf8(ref e) => e.description(),
            Custom(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        use Error::*;

        match *self {
            Gzip(ref e) => Some(e),
            Http(_) | TimedOut => None,
            Hyper(ref e) => Some(e),
            Tls(ref e) => Some(e),
            Utf8(ref e) => Some(e),
            Custom(ref e) => Some(&**e),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Error::*;

        match *self {
            Gzip(ref e) => Display::fmt(e, f),
            Http(ref code) => Display::fmt(code, f),
            Hyper(ref e) => Display::fmt(e, f),
            TimedOut => Display::fmt(self.description(), f),
            Tls(ref e) => Display::fmt(e, f),
            Utf8(ref e) => Display::fmt(e, f),
            Custom(ref e) => Display::fmt(e, f),
        }
    }
}
