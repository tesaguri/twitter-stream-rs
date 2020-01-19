//! Type aliases for use with `hyper` crate's HTTP client.

pub type FutureTwitterStream = crate::FutureTwitterStream<hyper_pkg::client::ResponseFuture>;
pub type Error = crate::Error<hyper_pkg::Error>;
pub type TwitterStream = crate::TwitterStream<hyper_pkg::Body>;
