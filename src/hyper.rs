//! Type aliases for use with `hyper` crate's HTTP client.

use hyper_pkg::client::{Client, HttpConnector};
use hyper_pkg::Body;
use hyper_tls::HttpsConnector;

pub type FutureTwitterStream =
    crate::FutureTwitterStream<Client<HttpsConnector<HttpConnector>>, Body>;
pub type Error = crate::Error<hyper_pkg::Error>;
pub type TwitterStream = crate::TwitterStream<hyper_pkg::Body>;
