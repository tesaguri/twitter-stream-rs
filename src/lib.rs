/*!
# Twitter Stream

A library for listening on Twitter Stream API.

## Usage

Add `twitter-stream` to your dependencies in your project's `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.1"
```

and this to your crate root:

```rust,no_run
extern crate twitter_stream;
```

## Overview

Here is a basic example that prints each Tweet's text from User Stream:

```rust,no_run
extern crate futures;
extern crate twitter_stream;
use futures::{Future, Stream};
use twitter_stream::{StreamMessage, TwitterStream};

# fn main() {
let consumer_key = "...";
let consumer_secret = "...";
let token = "...";
let token_secret = "...";

let stream = TwitterStream::user(consumer_key, consumer_secret, token, token_secret).unwrap();

stream
    .filter_map(|msg| {
        if let StreamMessage::Tweet(tweet) = msg {
            Some(tweet.text)
        } else {
            None
        }
    })
    .for_each(|tweet| {
        println!("{}", tweet);
        Ok(())
    })
    .wait().unwrap();
# }
```

In the example above, `stream` disconnects and returns error when a JSON message from Stream was failed to parse.
If you don't want this behavior, you can opt to parse the messages manually:

```rust,no_run
# extern crate futures;
# extern crate twitter_stream;
extern crate serde_json;

# use futures::{Future, Stream};
use twitter_stream::{StreamMessage, TwitterJsonStream};

# fn main() {
# let (consumer_key, consumer_secret, token, token_secret) = ("", "", "", "");
let stream = TwitterJsonStream::user(consumer_key, consumer_secret, token, token_secret).unwrap();

stream
    .filter_map(|json| {
        if let Ok(StreamMessage::Tweet(tweet)) = serde_json::from_str(&json) {
            Some(tweet.text)
        } else {
            None
        }
    })
    .for_each(|tweet| {
        println!("{}", tweet);
        Ok(())
    })
    .wait().unwrap();
# }
*/

extern crate chrono;
extern crate flate2;
extern crate futures;
extern crate hyper;
extern crate oauthcli;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json as json;
extern crate url;

#[macro_use]
mod util;

pub mod direct_message;
pub mod entities;
pub mod geometry;
pub mod list;
pub mod message;
pub mod place;
pub mod tweet;
pub mod types;
pub mod user;

pub use direct_message::DirectMessage;
pub use entities::Entities;
pub use geometry::Geometry;
pub use list::List;
pub use message::StreamMessage;
pub use place::Place;
pub use tweet::Tweet;
pub use user::User;

use futures::{Async, Future, Poll, Stream};
use hyper::client::Client;
use hyper::header::{Headers, AcceptEncoding, Authorization, ContentEncoding, ContentType, Encoding, UserAgent, qitem};
use message::Disconnect;
use oauthcli::{OAuthAuthorizationHeaderBuilder, SignatureMethod};
use util::{Lines, OAuthHeaderWrapper, Timeout};
use std::convert::From;
use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufReader};
use std::result;
use std::time::{Duration, Instant};
use types::{FilterLevel, JsonError, RequestMethod, StatusCode, UrlError, With};
use url::Url;
use url::form_urlencoded::{Serializer, Target};
use user::UserId;

macro_rules! def_stream {
    (
        $(#[$builder_attr:meta])*
        pub struct $B:ident<$lifetime:tt> {
            $($b_field:ident: $bf_ty:ty),*;
            $(
                $(#[$setter_attr:meta])*
                :$setter:ident: $s_ty:ty = $default:expr
            ),*;
            $(
                $(#[$o_attr:meta])*
                :$option:ident: Option<$o_ty:ty>
            ),*;
        }

        $(#[$stream_attr:meta])*
        pub struct $S:ident {
            $($s_field:ident: $sf_ty:ty,)*
        }

        $(#[$json_stream_attr:meta])*
        pub struct $JS:ident {
            $($js_field:ident: $jsf_ty:ty,)*
        }

        $(
            $(#[$constructor_attr:meta])*
            -
            $(#[$s_constructor_attr:meta])*
            -
            $(#[$js_constructor_attr:meta])*
            pub fn $constructor:ident($Method:ident, $end_point:expr);
        )*
    ) => {
        $(#[$builder_attr])*
        pub struct $B<$lifetime> {
            $($b_field: $bf_ty,)*
            $($setter: $s_ty,)*
            $($option: Option<$o_ty>,)*
        }

        $(#[$stream_attr])*
        pub struct $S {
            $($s_field: $sf_ty,)*
        }

        $(#[$json_stream_attr])*
        pub struct $JS {
            $($js_field: $jsf_ty,)*
        }

        impl<$lifetime> $B<$lifetime> {
            /// Constructs a builder for a Stream at a custom end point.
            pub fn custom($($b_field: $bf_ty),*) -> Self {
                $B {
                    $($b_field: $b_field,)*
                    $($setter: $default,)*
                    $($option: None,)*
                }
            }

            $(
                $(#[$constructor_attr])*
                pub fn $constructor(consumer_key: &$lifetime str, consumer_secret: &$lifetime str,
                    token: &$lifetime str, token_secret: &$lifetime str) -> Self
                {
                    $B::custom(RequestMethod::$Method, $end_point, consumer_key, consumer_secret, token, token_secret)
                }
            )*

            $(
                $(#[$setter_attr])*
                pub fn $setter(&mut self, $setter: $s_ty) -> &mut Self {
                    self.$setter = $setter;
                    self
                }
            )*

            $(
                $(#[$o_attr])*
                pub fn $option<T: Into<Option<$o_ty>>>(&mut self, $option: T) -> &mut Self {
                    self.$option = $option.into();
                    self
                }
            )*
        }

        impl $S {
            $(
                $(#[$s_constructor_attr])*
                pub fn $constructor<'a>(consumer_key: &'a str, consumer_secret: &'a str,
                    token: &'a str, token_secret: &'a str) -> Result<Self>
                {
                    $B::$constructor(consumer_key, consumer_secret, token, token_secret).listen()
                }
            )*
        }

        impl $JS {
            $(
                $(#[$js_constructor_attr])*
                pub fn $constructor<'a>(consumer_key: &'a str, consumer_secret: &'a str,
                    token: &'a str, token_secret: &'a str) -> Result<Self>
                {
                    $B::$constructor(consumer_key, consumer_secret, token, token_secret).listen_json()
                }
            )*
        }
    };
}

def_stream! {
    /// A builder for `TwitterStream`.
    #[derive(Clone, Debug)]
    pub struct TwitterStreamBuilder<'a> {
        method: RequestMethod,
        end_point: &'a str,
        consumer_key: &'a str,
        consumer_secret: &'a str,
        token: &'a str,
        token_secret: &'a str;

        // Setters:

        /// Set a timeout for the stream. The default is 90 secs.
        :timeout: Duration = Duration::from_secs(90),

        // Setters of API parameters:

        // delimited: bool,

        /// Set whether to receive messages when in danger of being disconnected.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#stallwarnings
        :stall_warnings: bool = false,

        /// Set the minimum `filter_level` Tweet attribute to receive. The default is `FilterLevel::None`.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#filter_level
        :filter_level: FilterLevel = FilterLevel::None,

        /// Set whether to receive all @replies.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#replies
        :replies: bool = false;

        // stringify_friend_ids: bool,

        // Optional setters:

        /// Set a custom `hyper::client::Client` object to use when connecting to the Stream.
        :client: Option<&'a Client>,

        /// Set a user agent string to be sent when connectiong to the Stream.
        :user_agent: Option<&'a str>,

        // Optional setters for API parameters:

        /// Set a comma-separated language identifiers to only receive Tweets written in the specified languages.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#language
        :language: Option<&'a str>,

        /// Set a list of user IDs to receive Tweets only from the specified users.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1] https://dev.twitter.com/streaming/overview/request-parameters#follow
        :follow: Option<&'a [UserId]>,

        /// A comma separated list of phrases to filter Tweets by.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#track
        :track: Option<&'a str>,

        /// Set a list of bounding boxes to filter Tweets by, specified by a pair of coordinates in
        /// the form of ((longitude, latitude), (longitude, latitude)) tuple.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#locations
        :locations: Option<&'a [((f64, f64), (f64, f64))]>,

        /// The `count` parameter. This parameter requires elevated access to use.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#count
        :count: Option<i32>,

        /// Set types of messages delivered to User and Site Streams clients.
        :with: Option<With>;
    }

    /// A listener for Twitter Stream API.
    pub struct TwitterStream {
        inner: TwitterJsonStream,
    }

    /// Same as `TwitterStream` except that it yields raw JSON string messages.
    pub struct TwitterJsonStream {
        lines: Lines,
        timeout: Duration,
        timer: Timeout,
    }

    // Constructors for `TwitterStreamBuilder`:

    /// Create a builder for `POST statuses/filter`.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/post/statuses/filter
    -
    /// A shorthand for `TwitterStreamBuilder::filter().listen()`.
    -
    /// A shorthand for `TwitterStreamBuilder::filter().listen_json()`.
    pub fn filter(Post, "https://stream.twitter.com/1.1/statuses/filter.json");

    /// Create a builder for `GET statuses/sample`.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/sample
    -
    /// A shorthand for `TwitterStreamBuilder::sample().listen()`.
    -
    /// A shorthand for `TwitterStreamBuilder::sample().listen_json()`.
    pub fn sample(Get, "https://stream.twitter.com/1.1/statuses/sample.json");

    /// Create a builder for `GET statuses/firehose`. This endpoint requires special permission to access.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/firehose
    -
    /// A shorthand for `TwitterStreamBuilder::firehose().listen()`.
    -
    /// A shorthand for `TwitterStreamBuilder::firehose().listen_json()`.
    pub fn firehose(Get, "https://stream.twitter.com/1.1/statuses/firehose.json");

    /// Create a builder for `GET user` (a.k.a. User Stream).
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/get/user
    -
    /// A shorthand for `TwitterStreamBuilder::user().listen()`.
    -
    /// A shorthand for `TwitterStreamBuilder::user().listen_json()`.
    pub fn user(Get, "https://userstream.twitter.com/1.1/user.json");

    /// Create a builder for `GET site` (a.k.a. Site Stream).
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/get/site
    -
    /// A shorthand for `TwitterStreamBuilder::site().listen()`.
    -
    /// A shorthand for `TwitterStreamBuilder::site().listen_json()`.
    pub fn site(Get, "https://sitestream.twitter.com/1.1/site.json");
}

/// An error occurred while trying to connect to the Stream API.
#[derive(Debug)]
pub enum Error {
    /// An error occured while parsing the gzip header of the response from the server.
    Gzip(io::Error),
    /// An HTTP error from the Stream.
    Http(StatusCode),
    /// An error from the `hyper` crate.
    Hyper(hyper::Error),
    /// An invalid url was passed to `TwitterStreamBuilder::custom` method.
    Url(url::ParseError),
    #[cfg(feature = "tls-failable")]
    /// An error returned from a TLS client.
    Tls(default_client::Error),
}

/// An error occured while listening on the Stream API.
#[derive(Debug)]
pub enum StreamError {
    /// The Stream has been disconnected by the server.
    Disconnect(Disconnect),
    /// An error from the Stream.
    Stream(JsonStreamError),
    /// Failed to parse a JSON message from Stream API.
    Json(JsonError),
}

/// An error occured while listening on the Stream API through `TwitterJsonStream`.
#[derive(Debug)]
pub enum JsonStreamError {
    /// An I/O error.
    Io(io::Error),
    /// The Stream has timed out.
    TimedOut(u64),
}

pub type Result<T> = result::Result<T, Error>;
pub type StreamResult<T> = result::Result<T, StreamError>;
pub type JsonStreamResult<T> = result::Result<T, JsonStreamError>;

impl<'a> TwitterStreamBuilder<'a> {
    /// Attempt to start listening on the Stream API and returns a `Stream` which yields parsed messages from the API.
    pub fn listen(&self) -> Result<TwitterStream> {
        Ok(TwitterStream {
            inner: self.listen_json()?,
        })
    }

    /// Attempt to start listening on the Stream API and returns a `Stream` which yields JSON messages from the API.
    pub fn listen_json(&self) -> Result<TwitterJsonStream> {
        Ok(TwitterJsonStream {
            lines: self.connect()?,
            timeout: self.timeout,
            timer: Timeout::after(self.timeout),
        })
    }

    /// Attempt to make an HTTP connection to the end point of the Stream API.
    fn connect(&self) -> Result<Lines> {
        let mut url = Url::parse(self.end_point)?;

        let mut headers = Headers::new();
        headers.set(AcceptEncoding(vec![qitem(Encoding::Chunked), qitem(Encoding::Gzip)]));
        if let Some(ua) = self.user_agent {
            headers.set(UserAgent(ua.to_owned()));
        }

        // Holds a borrowed or owned value.
        enum Hold<'a, T: 'a> {
            Borrowed(&'a T),
            Owned(T),
        }

        impl<'a, T: 'a> std::ops::Deref for Hold<'a, T> {
            type Target = T;
            fn deref(&self) -> &T {
                match *self {
                    Hold::Borrowed(t) => t,
                    Hold::Owned(ref t) => t,
                }
            }
        }

        let client = if let Some(c) = self.client {
            Hold::Borrowed(c)
        } else {
            #[cfg(feature = "tls-failable")]
            {
                default_client::new().map(Hold::Owned).map_err(Error::Tls)?
            }
            #[cfg(not(feature = "tls-failable"))]
            {
                Hold::Owned(default_client::new())
            }
        };

        let res = if RequestMethod::Post == self.method {
            use hyper::mime::{Mime, SubLevel, TopLevel};

            headers.set(ContentType(Mime(TopLevel::Application, SubLevel::WwwFormUrlEncoded, Vec::new())));
            let mut body = Serializer::new(String::new());
            self.append_query_pairs(&mut body);
            let body = body.finish();
            headers.set(self.create_authorization_header(&url, Some(body.as_ref())));
            client
                .post(url)
                .headers(headers)
                .body(&body)
                .send()?
        } else {
            self.append_query_pairs(&mut url.query_pairs_mut());
            headers.set(self.create_authorization_header(&url, None));
            client
                .request(self.method.clone(), url)
                .headers(headers)
                .send()?
        };

        if StatusCode::Ok == res.status {
            if res.headers.get::<ContentEncoding>()
                .map_or(false, |&ContentEncoding(ref v)| v.contains(&Encoding::Gzip))
            {
                use flate2::read::GzDecoder;
                let res = GzDecoder::new(res).map_err(Error::Gzip)?;
                Ok(util::lines(BufReader::new(res)))
            } else {
                Ok(util::lines(BufReader::new(res)))
            }
        } else {
            Err(res.status.into())
        }
    }

    fn append_query_pairs<T: Target>(&self, pairs: &mut Serializer<T>) {
        if self.stall_warnings {
            pairs.append_pair("stall_warnings", "true");
        }
        if self.filter_level != FilterLevel::None {
            pairs.append_pair("filter_level", self.filter_level.as_ref());
        }
        if let Some(s) = self.language {
            pairs.append_pair("language", s);
        }
        if let Some(ids) = self.follow {
            let mut val = String::new();
            if let Some(id) = ids.first() {
                val = id.to_string();
            }
            for id in ids.into_iter().skip(1) {
                val.push(',');
                val.push_str(&id.to_string());
            }
            pairs.append_pair("follow", &val);
        }
        if let Some(s) = self.track {
            pairs.append_pair("track", s);
        }
        if let Some(locs) = self.locations {
            let mut val = String::new();
            macro_rules! push {
                ($coordinate:expr) => {{
                    val.push(',');
                    val.push_str(&$coordinate.to_string());
                }};
            }
            if let Some(&((lon1, lat1), (lon2, lat2))) = locs.first() {
                val = lon1.to_string();
                push!(lat1);
                push!(lon2);
                push!(lat2);
            }
            for &((lon1, lat1), (lon2, lat2)) in locs.into_iter().skip(1) {
                push!(lon1);
                push!(lat1);
                push!(lon2);
                push!(lat2);
            }
            pairs.append_pair("locations", &val);
        }
        if let Some(n) = self.count {
            pairs.append_pair("count", &n.to_string());
        }
        if let Some(ref w) = self.with {
            pairs.append_pair("with", w.as_ref());
        }
        if self.replies {
            pairs.append_pair("replies", "all");
        }
    }

    fn create_authorization_header(&self, url: &Url, params: Option<&[u8]>) -> Authorization<OAuthHeaderWrapper>
    {
        use url::form_urlencoded;

        let mut oauth = OAuthAuthorizationHeaderBuilder::new(
            self.method.as_ref(), url, self.consumer_key, self.consumer_secret, SignatureMethod::HmacSha1
        );
        oauth.token(self.token, self.token_secret);
        if let Some(p) = params {
            oauth.request_parameters(form_urlencoded::parse(p));
        }
        let oauth = oauth.finish_for_twitter();

        Authorization(OAuthHeaderWrapper(oauth))
    }
}

impl Stream for TwitterStream {
    type Item = StreamMessage;
    type Error = StreamError;

    fn poll(&mut self) -> Poll<Option<StreamMessage>, StreamError> {
        use Async::*;

        match self.inner.poll()? {
            Ready(Some(line)) => match json::from_str(&line)? {
                StreamMessage::Disconnect(d) => Err(StreamError::Disconnect(d)),
                msg => Ok(Ready(Some(msg))),
            },
            Ready(None) => Ok(Ready(None)),
            NotReady => Ok(NotReady),
        }
    }
}

impl Stream for TwitterJsonStream {
    type Item = String;
    type Error = JsonStreamError;

    fn poll(&mut self) -> Poll<Option<String>, JsonStreamError> {
        use Async::*;

        loop {
            match self.lines.poll()? {
                Ready(Some(line)) => {
                    let now = Instant::now();
                    self.timer = Timeout::after(self.timeout);
                    self.timer.park(now);

                    if !line.is_empty() {
                        return Ok(Ready(Some(line)));
                    }
                },
                Ready(None) => return Ok(None.into()),
                NotReady => {
                    if let Ok(Ready(())) = self.timer.poll() {
                        return Err(JsonStreamError::TimedOut(self.timeout.as_secs()));
                    } else {
                        return Ok(NotReady);
                    }
                },
            }
        }
    }
}

impl IntoIterator for TwitterStream {
    type Item = StreamResult<StreamMessage>;
    type IntoIter = futures::stream::Wait<Self>;

    fn into_iter(self) -> Self::IntoIter {
        self.wait()
    }
}

impl IntoIterator for TwitterJsonStream {
    type Item = JsonStreamResult<String>;
    type IntoIter = futures::stream::Wait<Self>;

    fn into_iter(self) -> Self::IntoIter {
        self.wait()
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use Error::*;

        match *self {
            Gzip(ref e) => e.description(),
            Http(ref status) => status.canonical_reason().unwrap_or("<unknown status code>"),
            Hyper(ref e) => e.description(),
            Url(ref e) => e.description(),
            #[cfg(feature = "tls-failable")]
            Tls(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use Error::*;

        match *self {
            Gzip(ref e) => Some(e),
            Http(_) => None,
            Hyper(ref e) => Some(e),
            Url(ref e) => Some(e),
            #[cfg(feature = "tls-failable")]
            Tls(ref e) => Some(e),
        }
    }
}

impl StdError for StreamError {
    fn description(&self) -> &str {
        use StreamError::*;

        match *self {
            Disconnect(ref d) => &d.reason,
            Stream(ref e) => e.description(),
            Json(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use StreamError::*;

        match *self {
            Disconnect(_) => None,
            Stream(ref e) => Some(e),
            Json(ref e) => Some(e),
        }
    }
}

impl StdError for JsonStreamError {
    fn description(&self) -> &str {
        use JsonStreamError::*;

        match *self {
            Io(ref e) => e.description(),
            TimedOut(_) => "timed out",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use JsonStreamError::*;

        match *self {
            Io(ref e) => Some(e),
            TimedOut(_) => None,
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
            Url(ref e) => Display::fmt(e, f),
            #[cfg(feature = "tls-failable")]
            Tls(ref e) => Display::fmt(e, f),
        }
    }
}

impl Display for StreamError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use StreamError::*;

        match *self {
            Disconnect(ref d) => Display::fmt(d, f),
            Stream(ref e) => Display::fmt(e, f),
            Json(ref e) => Display::fmt(e, f),
        }
    }
}

impl Display for JsonStreamError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use JsonStreamError::*;

        match *self {
            Io(ref e) => Display::fmt(e, f),
            TimedOut(timeout) => write!(f, "connection timed out after {} sec", timeout),
        }
    }
}

impl From<StatusCode> for Error {
    fn from(e: StatusCode) -> Self {
        Error::Http(e)
    }
}

impl From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Self {
        Error::Hyper(e)
    }
}

impl From<UrlError> for Error {
    fn from(e: UrlError) -> Self {
        Error::Url(e)
    }
}

impl From<JsonError> for StreamError {
    fn from(e: JsonError) -> Self {
        StreamError::Json(e)
    }
}

impl From<JsonStreamError> for StreamError {
    fn from(e: JsonStreamError) -> Self {
        StreamError::Stream(e)
    }
}

impl From<io::Error> for JsonStreamError {
    fn from(e: io::Error) -> Self {
        JsonStreamError::Io(e)
    }
}

#[cfg(feature = "hyper-native-tls")]
mod default_client {
    extern crate hyper_native_tls;
    extern crate native_tls;

    use hyper::client::Client;
    use hyper::net::HttpsConnector;

    pub type Error = native_tls::Error;

    pub fn new() -> Result<Client, Error> {
        hyper_native_tls::NativeTlsClient::new()
            .map(HttpsConnector::new)
            .map(Client::with_connector)
    }
}

#[cfg(feature = "hyper-openssl")]
mod default_client {
    extern crate hyper_openssl;
    extern crate openssl;

    use hyper::client::Client;
    use hyper::net::HttpsConnector;

    pub type Error = openssl::error::ErrorStack;

    pub fn new() -> Result<Client, Error> {
        hyper_openssl::OpensslClient::new()
            .map(HttpsConnector::new)
            .map(Client::with_connector)
    }
}

#[cfg(feature = "hyper-rustls")]
mod default_client {
    extern crate hyper_rustls;

    use hyper::client::Client;
    use hyper::net::HttpsConnector;

    pub fn new() -> Client {
        Client::with_connector(HttpsConnector::new(hyper_rustls::TlsClient::new()))
    }
}

#[cfg(not(feature = "tls"))]
mod default_client {
    use hyper::client::Client;

    pub fn new() -> Client {
        Client::new()
    }
}
