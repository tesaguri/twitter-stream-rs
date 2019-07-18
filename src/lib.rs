#![doc(html_root_url = "https://docs.rs/twitter-stream/0.9.0")]
#![recursion_limit = "128"]

/*!
# Twitter Stream

A library for listening on Twitter Streaming API.

## Usage

Add `twitter-stream` to your dependencies in your project's `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.9"
```

## Overview

Here is a basic example that prints public mentions to @Twitter in JSON format:

```rust,no_run
use twitter_stream::{Token, TwitterStreamBuilder};
use twitter_stream::rt::{self, Future, Stream};

# fn main() {
let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

let future = TwitterStreamBuilder::filter(token)
    .track(Some("@Twitter"))
    .listen()
    .unwrap()
    .flatten_stream()
    .for_each(|json| {
        println!("{}", json);
        Ok(())
    })
    .map_err(|e| println!("error: {}", e));

rt::run(future);
# }
```
*/

#[macro_use]
mod util;

pub mod error;
#[cfg(feature = "runtime")]
pub mod rt;
pub mod types;

mod gzip;
mod token;

pub use crate::error::Error;
pub use crate::token::Token;

use std::borrow::Borrow;
use std::time::Duration;

use bytes::Bytes;
use futures::{try_ready, Async, Future, Poll, Stream};
use http::response::Parts;
use hyper::body::{Body, Payload};
use hyper::client::connect::Connect;
use hyper::client::{Client, ResponseFuture};
use hyper::header::{
    HeaderValue, ACCEPT_ENCODING, AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE,
};
use hyper::Request;
use oauth::OAuth1Authorize;
use oauth1_request_derive::OAuth1Authorize;
use string::TryFrom;

use crate::gzip::MaybeGzip;
use crate::types::{FilterLevel, RequestMethod, StatusCode, Uri};
use crate::util::*;

/// A builder for `TwitterStream`.
///
/// ## Example
///
/// ```rust,no_run
/// use twitter_stream::{Token, TwitterStreamBuilder};
/// use twitter_stream::rt::{self, Future, Stream};
///
/// # fn main() {
/// let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");
///
/// let future = TwitterStreamBuilder::sample(token)
///     .timeout(None)
///     .listen()
///     .unwrap()
///     .flatten_stream()
///     .for_each(|json| {
///         println!("{}", json);
///         Ok(())
///     })
///     .map_err(|e| println!("error: {}", e));
///
/// rt::run(future);
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct TwitterStreamBuilder<'a, T = Token> {
    method: RequestMethod,
    endpoint: Uri,
    token: T,
    inner: BuilderInner<'a>,
}

/// A future returned by constructor methods
/// which resolves to a `TwitterStream`.
pub struct FutureTwitterStream {
    response: MaybeTimeout<ResponseFuture>,
}

/// A listener for Twitter Streaming API.
/// It yields JSON strings returned from the API.
pub struct TwitterStream {
    inner: Lines<MaybeGzip<MaybeTimeoutStream<Body>>>,
}

#[derive(Clone, Debug, OAuth1Authorize)]
struct BuilderInner<'a> {
    #[oauth1(skip)]
    timeout: Option<Duration>,
    #[oauth1(skip_if = "not")]
    stall_warnings: bool,
    #[oauth1(option)]
    filter_level: Option<FilterLevel>,
    #[oauth1(option)]
    language: Option<&'a str>,
    #[oauth1(option, encoded, fmt = "fmt_follow")]
    follow: Option<&'a [u64]>,
    #[oauth1(option)]
    track: Option<&'a str>,
    #[oauth1(encoded, option, fmt = "fmt_locations")]
    #[allow(clippy::type_complexity)]
    locations: Option<&'a [((f64, f64), (f64, f64))]>,
    #[oauth1(encoded, option)]
    count: Option<i32>,
}

impl<'a, C, A> TwitterStreamBuilder<'a, Token<C, A>>
where
    C: Borrow<str>,
    A: Borrow<str>,
{
    /// Create a builder for `POST statuses/filter` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/post/statuses/filter
    pub fn filter(token: Token<C, A>) -> Self {
        const URI: &str = "https://stream.twitter.com/1.1/statuses/filter.json";
        Self::custom(RequestMethod::POST, Uri::from_static(URI), token)
    }

    /// Create a builder for `GET statuses/sample` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/sample
    pub fn sample(token: Token<C, A>) -> Self {
        const URI: &str = "https://stream.twitter.com/1.1/statuses/sample.json";
        Self::custom(RequestMethod::GET, Uri::from_static(URI), token)
    }

    /// Constructs a builder for a Stream at a custom endpoint.
    pub fn custom(method: RequestMethod, endpoint: Uri, token: Token<C, A>) -> Self {
        Self {
            method,
            endpoint,
            token,
            inner: BuilderInner {
                timeout: Some(Duration::from_secs(90)),
                stall_warnings: false,
                filter_level: None,
                language: None,
                follow: None,
                track: None,
                locations: None,
                count: None,
            },
        }
    }

    /// Start listening on the Streaming API endpoint, returning a `Future` which resolves
    /// to a `Stream` yielding JSON messages from the API.
    #[cfg(feature = "tls")]
    pub fn listen(&self) -> Result<FutureTwitterStream, error::TlsError> {
        let conn = hyper_tls::HttpsConnector::new(1)?;
        Ok(self.listen_with_client(&Client::builder().build::<_, Body>(conn)))
    }

    /// Same as `listen` except that it uses `client` to make HTTP request to the endpoint.
    pub fn listen_with_client<Conn, B>(&self, client: &Client<Conn, B>) -> FutureTwitterStream
    where
        Conn: Connect + Sync + 'static,
        Conn::Transport: 'static,
        Conn::Future: 'static,
        B: Default + From<Vec<u8>> + Payload + Send + 'static,
        B::Data: Send,
    {
        let mut req = Request::builder();
        req.method(self.method.clone())
            .header(ACCEPT_ENCODING, HeaderValue::from_static("gzip"));

        let req = if RequestMethod::POST == self.method {
            let oauth::Request {
                authorization,
                data,
            } = self.inner.authorize_form(
                "POST",
                &self.endpoint,
                self.token.consumer_key.borrow(),
                self.token.consumer_secret.borrow(),
                self.token.access_secret.borrow(),
                oauth::HmacSha1,
                &*oauth::Options::new().token(self.token.access_key.borrow()),
            );

            req.uri(self.endpoint.clone())
                .header(AUTHORIZATION, Bytes::from(authorization))
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/x-www-form-urlencoded"),
                )
                .header(CONTENT_LENGTH, Bytes::from(data.len().to_string()))
                .body(data.into_bytes().into())
                .unwrap()
        } else {
            let oauth::Request {
                authorization,
                data: uri,
            } = self.inner.authorize(
                self.method.as_ref(),
                &self.endpoint,
                self.token.consumer_key.borrow(),
                self.token.consumer_secret.borrow(),
                self.token.access_secret.borrow(),
                oauth::HmacSha1,
                &*oauth::Options::new().token(self.token.access_key.borrow()),
            );

            req.uri(uri)
                .header(AUTHORIZATION, Bytes::from(authorization))
                .body(B::default())
                .unwrap()
        };

        let res = client.request(req);
        FutureTwitterStream {
            response: timeout(res, self.inner.timeout),
        }
    }
}

impl<'a, C, A> TwitterStreamBuilder<'a, Token<C, A>> {
    /// Reset the HTTP request method to be used when connecting
    /// to the server.
    pub fn method(&mut self, method: RequestMethod) -> &mut Self {
        self.method = method;
        self
    }

    /// Reset the API endpoint URI to be connected.
    pub fn endpoint(&mut self, endpoint: Uri) -> &mut Self {
        self.endpoint = endpoint;
        self
    }

    /// Reset the token to be used to log into Twitter.
    pub fn token(&mut self, token: Token<C, A>) -> &mut Self {
        self.token = token;
        self
    }

    /// Set a timeout for the stream.
    ///
    /// Passing `None` disables the timeout.
    ///
    /// Default is 90 seconds.
    pub fn timeout(&mut self, timeout: impl Into<Option<Duration>>) -> &mut Self {
        self.inner.timeout = timeout.into();
        self
    }

    /// Set whether to receive messages when in danger of
    /// being disconnected.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#stall-warnings
    pub fn stall_warnings(&mut self, stall_warnings: bool) -> &mut Self {
        self.inner.stall_warnings = stall_warnings;
        self
    }

    /// Set the minimum `filter_level` Tweet attribute to receive.
    /// The default is `FilterLevel::None`.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#filter-level
    pub fn filter_level(&mut self, filter_level: impl Into<Option<FilterLevel>>) -> &mut Self {
        self.inner.filter_level = filter_level.into();
        self
    }

    /// Set a comma-separated language identifiers to receive Tweets
    /// written in the specified languages only.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#language
    pub fn language(&mut self, language: impl Into<Option<&'a str>>) -> &mut Self {
        self.inner.language = language.into();
        self
    }

    /// Set a list of user IDs to receive Tweets from the specified users.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#follow
    pub fn follow(&mut self, follow: impl Into<Option<&'a [u64]>>) -> &mut Self {
        self.inner.follow = follow.into();
        self
    }

    /// A comma separated list of phrases to filter Tweets by.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#track
    pub fn track(&mut self, track: impl Into<Option<&'a str>>) -> &mut Self {
        self.inner.track = track.into();
        self
    }

    /// Set a list of bounding boxes to filter Tweets by,
    /// specified by a pair of coordinates in the form of
    /// `((longitude, latitude), (longitude, latitude))` tuple.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#locations
    pub fn locations(
        &mut self,
        locations: impl Into<Option<&'a [((f64, f64), (f64, f64))]>>,
    ) -> &mut Self {
        self.inner.locations = locations.into();
        self
    }

    /// The `count` parameter.
    /// This parameter requires elevated access to use.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#count
    pub fn count(&mut self, count: impl Into<Option<i32>>) -> &mut Self {
        self.inner.count = count.into();
        self
    }
}

#[cfg(feature = "tls")]
impl TwitterStream {
    /// A shorthand for `TwitterStreamBuilder::filter().listen()`.
    pub fn filter<C, A>(token: Token<C, A>) -> Result<FutureTwitterStream, error::TlsError>
    where
        C: Borrow<str>,
        A: Borrow<str>,
    {
        TwitterStreamBuilder::filter(token).listen()
    }

    /// A shorthand for `TwitterStreamBuilder::sample().listen()`.
    pub fn sample<C, A>(token: Token<C, A>) -> Result<FutureTwitterStream, error::TlsError>
    where
        C: Borrow<str>,
        A: Borrow<str>,
    {
        TwitterStreamBuilder::sample(token).listen()
    }
}

impl Future for FutureTwitterStream {
    type Item = TwitterStream;
    type Error = Error;

    fn poll(&mut self) -> Poll<TwitterStream, Error> {
        let res = try_ready!(self.response.poll());
        let (parts, body) = res.into_parts();
        let Parts {
            status, headers, ..
        } = parts;

        if StatusCode::OK != status {
            return Err(Error::Http(status));
        }

        let body = timeout_to_stream(&self.response, body);
        let use_gzip = headers
            .get_all(CONTENT_ENCODING)
            .iter()
            .any(|e| e == "gzip");
        let inner = if use_gzip {
            Lines::new(MaybeGzip::gzip(body))
        } else {
            Lines::new(MaybeGzip::identity(body))
        };

        Ok(TwitterStream { inner }.into())
    }
}

impl Stream for TwitterStream {
    type Item = string::String<Bytes>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<string::String<Bytes>>, Error> {
        loop {
            match self.inner.poll() {
                Ok(Async::Ready(Some(line))) => {
                    if line.iter().all(|&c| is_json_whitespace(c)) {
                        continue;
                    }

                    // TODO: change the return type to `std::string::String` in v0.10.
                    let line = Bytes::from(line);
                    let line = string::String::<Bytes>::try_from(line).map_err(Error::Utf8)?;
                    return Ok(Async::Ready(Some(line)));
                }
                Ok(Async::Ready(None)) => return Ok(Async::Ready(None)),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => match *self.inner.get_mut() {
                    Either::A(ref mut gzip) => {
                        if let Some(e) = gzip.get_mut().take_err() {
                            return Err(e);
                        } else {
                            return Err(Error::Gzip(e));
                        }
                    }
                    Either::B(ref mut stream_read) => return Err(stream_read.take_err().unwrap()),
                },
            }
        }
    }
}

fn is_json_whitespace(c: u8) -> bool {
    // RFC7159 §2
    b" \t\n\r".contains(&c)
}
