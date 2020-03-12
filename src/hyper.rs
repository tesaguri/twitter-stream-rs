//! Type aliases for use with [`hyper`](hyper_pkg) crate's HTTP client.

/// A type alias of [`FutureTwitterStream`](crate::FutureTwitterStream) using Hyper's HTTP client.
pub type FutureTwitterStream = crate::FutureTwitterStream<hyper_pkg::client::ResponseFuture>;
/// A type alias of [`Error`](crate::error::Error)
/// whose `Service` variant contains [`hyper::Error`](hyper_pkg::Error).
pub type Error = crate::Error<hyper_pkg::Error>;
/// A type alias of [`TwitterStream`](crate::TwitterStream) using Hyper's HTTP client.
pub type TwitterStream = crate::TwitterStream<hyper_pkg::Body>;
