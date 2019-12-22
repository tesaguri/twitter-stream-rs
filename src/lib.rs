#![doc(html_root_url = "https://docs.rs/twitter-stream/0.10.0-alpha.4")]

/*!
# Twitter Stream

A library for listening on Twitter Streaming API.

## Usage

Add `twitter-stream` to your dependencies in your project's `Cargo.toml`:

```toml
[dependencies]
futures = "0.3"
tokio = { version = "0.2", features = ["macros"] }
twitter-stream = "=0.10.0-alpha.4"
```

## Overview

Here is a basic example that prints public mentions to @Twitter in JSON format:

```rust,no_run
use futures::prelude::*;
use twitter_stream::Token;

# #[tokio::main]
# async fn main() {
let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

twitter_stream::Builder::filter(token)
    .track(Some("@Twitter"))
    .listen()
    .try_flatten_stream()
    .try_for_each(|json| {
        println!("{}", json);
        future::ok(())
    })
    .await
    .unwrap();
# }
```
*/

#[cfg(feature = "hyper")]
extern crate hyper_pkg;

#[macro_use]
mod util;

pub mod error;
#[cfg(feature = "hyper")]
pub mod hyper;
pub mod types;

mod gzip;
mod token;

pub use oauth::Credentials;

pub use crate::error::Error;
pub use crate::token::Token;

use std::borrow::Borrow;
use std::future::Future;
use std::pin::Pin;
use std::str;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_core::Stream;
use futures_util::ready;
use http::header::{
    HeaderValue, ACCEPT_ENCODING, AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE,
};
use http::response::Parts;
use http::{Request, Response};
use http_body::Body;
use pin_project::pin_project;
use tower_service::Service;

use crate::gzip::MaybeGzip;
use crate::types::{FilterLevel, RequestMethod, StatusCode, Uri};
use crate::util::*;

/// A builder for `TwitterStream`.
///
/// ## Example
///
/// ```rust,no_run
/// use futures::prelude::*;
/// use twitter_stream::Token;
///
/// # #[tokio::main]
/// # async fn main() {
/// let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");
///
/// twitter_stream::Builder::sample(token)
///     .listen()
///     .try_flatten_stream()
///     .try_for_each(|json| {
///         println!("{}", json);
///         future::ok(())
///     })
///     .await
///     .unwrap();
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Builder<'a, T = Token> {
    method: RequestMethod,
    endpoint: Uri,
    token: T,
    inner: BuilderInner<'a>,
}

/// A future returned by constructor methods
/// which resolves to a `TwitterStream`.
#[pin_project]
pub struct FutureTwitterStream<F> {
    #[pin]
    response: F,
}

/// A listener for Twitter Streaming API.
/// It yields JSON strings returned from the API.
#[pin_project]
pub struct TwitterStream<B: Body<Data = Bytes>> {
    #[pin]
    inner: Lines<MaybeGzip<HttpBodyAsStream<B>>>,
}

#[derive(Clone, Debug, oauth::Authorize)]
struct BuilderInner<'a> {
    #[oauth1(skip_if = "not")]
    stall_warnings: bool,
    filter_level: Option<FilterLevel>,
    language: Option<&'a str>,
    #[oauth1(encoded, fmt = "fmt_follow")]
    follow: Option<&'a [u64]>,
    track: Option<&'a str>,
    #[oauth1(encoded, fmt = "fmt_locations")]
    #[allow(clippy::type_complexity)]
    locations: Option<&'a [((f64, f64), (f64, f64))]>,
    #[oauth1(encoded)]
    count: Option<i32>,
}

impl<'a, C, A> Builder<'a, Token<C, A>>
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
    ///
    /// # Panics
    ///
    /// This will panic if the underlying HTTPS connector failed to initialize.
    #[cfg(feature = "hyper")]
    pub fn listen(&self) -> crate::hyper::FutureTwitterStream {
        let conn = hyper_tls::HttpsConnector::new();
        self.listen_with_client(hyper_pkg::Client::builder().build::<_, hyper_pkg::Body>(conn))
    }

    /// Same as `listen` except that it uses `client` to make HTTP request to the endpoint.
    ///
    /// # Panics
    ///
    /// This will call `<S as Service>::call` without checking for `<S as Service>::poll_ready`
    /// and may cause a panic if `client` is not ready to send an HTTP request yet.
    pub fn listen_with_client<S, ReqB, ResB>(&self, mut client: S) -> FutureTwitterStream<S::Future>
    where
        S: Service<Request<ReqB>, Response = Response<ResB>>,
        ReqB: Default + From<Vec<u8>>,
        ResB: Body<Data = Bytes>,
    {
        let req = Request::builder()
            .method(self.method.clone())
            .header(ACCEPT_ENCODING, HeaderValue::from_static("gzip"));

        let mut oauth = oauth::Builder::new(self.token.client.as_ref(), oauth::HmacSha1);
        oauth.token(self.token.token.as_ref());
        let req = if RequestMethod::POST == self.method {
            let oauth::Request {
                authorization,
                data,
            } = oauth.post_form(&self.endpoint, &self.inner);

            req.uri(self.endpoint.clone())
                .header(AUTHORIZATION, authorization)
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/x-www-form-urlencoded"),
                )
                .header(CONTENT_LENGTH, data.len())
                .body(data.into_bytes().into())
                .unwrap()
        } else {
            let oauth::Request {
                authorization,
                data: uri,
            } = oauth.build(self.method.as_ref(), &self.endpoint, &self.inner);

            req.uri(uri)
                .header(AUTHORIZATION, authorization)
                .body(ReqB::default())
                .unwrap()
        };

        let response = client.call(req);
        FutureTwitterStream { response }
    }
}

impl<'a, C, A> Builder<'a, Token<C, A>> {
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

#[cfg(feature = "hyper")]
impl<B: Body<Data = Bytes>> TwitterStream<B> {
    /// A shorthand for `Builder::filter().listen()`.
    ///
    /// # Panics
    ///
    /// This will panic if the underlying HTTPS connector failed to initialize.
    pub fn filter<C, A>(token: Token<C, A>) -> crate::hyper::FutureTwitterStream
    where
        C: Borrow<str>,
        A: Borrow<str>,
    {
        Builder::filter(token).listen()
    }

    /// A shorthand for `Builder::sample().listen()`.
    ///
    /// # Panics
    ///
    /// This will panic if the underlying HTTPS connector failed to initialize.
    pub fn sample<C, A>(token: Token<C, A>) -> crate::hyper::FutureTwitterStream
    where
        C: Borrow<str>,
        A: Borrow<str>,
    {
        Builder::sample(token).listen()
    }
}

impl<F, B, E> Future for FutureTwitterStream<F>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: Body<Data = Bytes>,
{
    type Output = Result<TwitterStream<B>, Error<E>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = match ready!(self.project().response.poll(cx)) {
            Ok(res) => res,
            Err(e) => return Poll::Ready(Err(Error::Service(e))),
        };
        let (parts, body) = res.into_parts();
        let Parts {
            status, headers, ..
        } = parts;

        if StatusCode::OK != status {
            return Poll::Ready(Err(Error::Http(status)));
        }

        let use_gzip = headers
            .get_all(CONTENT_ENCODING)
            .iter()
            .any(|e| e == "gzip");
        let inner = if use_gzip {
            Lines::new(gzip::gzip(HttpBodyAsStream(body)))
        } else {
            Lines::new(gzip::identity(HttpBodyAsStream(body)))
        };

        Poll::Ready(Ok(TwitterStream { inner }))
    }
}

impl<B> Stream for TwitterStream<B>
where
    B: Body<Data = Bytes>,
{
    type Item = Result<string::String<Bytes>, Error<B::Error>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            let line = ready_some!(this.inner.as_mut().poll_next(cx))?;
            if line.iter().all(|&c| is_json_whitespace(c)) {
                continue;
            }

            str::from_utf8(&line).map_err(Error::Utf8)?;
            let line = unsafe {
                // Safety:
                // - We have checked above that `line` is valid as UTF-8.
                // - `Bytes` satisfies the requirements of `string::StableAsRef` trait
                // (https://github.com/carllerche/string/pull/17)
                string::String::<Bytes>::from_utf8_unchecked(line)
            };
            return Poll::Ready(Some(Ok(line)));
        }
    }
}

fn is_json_whitespace(c: u8) -> bool {
    // RFC7159 §2
    b" \t\n\r".contains(&c)
}
