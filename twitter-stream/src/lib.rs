/*!
# Twitter tream

A library for listening on Twitter Streaming API.

## Usage

Add `twitter-stream` to your dependencies in your project's `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.6"
```

and this to your crate root:

```rust,no_run
extern crate twitter_stream;
```

## Overview

Here is a basic example that prints public mentions to @Twitter in JSON format:

```rust,no_run
extern crate twitter_stream;

use twitter_stream::{Token, TwitterStream};
use twitter_stream::rt::{self, Future, Stream};

# fn main() {
let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

let future = TwitterStream::filter(&token, "@Twitter")
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

#[cfg(not(feature = "runtime"))]
compile_error!("`runtime` feature must be enabled for now.");

extern crate bytes;
#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate futures;
extern crate hmac;
extern crate http;
extern crate hyper;
extern crate byteorder;
extern crate libflate;
extern crate percent_encoding;
extern crate rand;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
extern crate sha1;
extern crate tokio;
extern crate tokio_timer;

#[macro_use]
mod util;

pub mod error;
pub mod rt;
pub mod types;

mod default_connector;
mod gzip;
mod query_builder;
mod token;

pub use token::Token;
pub use error::Error;

use std::borrow::Borrow;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use bytes::Bytes;
use futures::{Future, Poll, Stream};
use http::response::Parts;
use hyper::Request;
use hyper::body::{Body, Payload};
use hyper::client::{Client, ResponseFuture};
use hyper::client::connect::Connect;
use hyper::header::{
    HeaderValue,
    ACCEPT_ENCODING,
    AUTHORIZATION,
    CONTENT_ENCODING,
    CONTENT_LENGTH,
    CONTENT_TYPE,
};

use error::TlsError;
use gzip::Gzip;
use query_builder::{QueryBuilder, QueryOutcome};
use types::{FilterLevel, JsonStr, RequestMethod, StatusCode, Uri};
use util::{EitherStream, JoinDisplay, Lines, Timeout, TimeoutStream};

macro_rules! def_builder {
    (
        $(#[$builder_attr:meta])*
        pub struct $B:ident<$lifetime:tt, $T:ident, $Cli:ident> {
            $client:ident: $cli_ty:ty = $cli_default:expr;
            $($arg:ident: $a_ty:ty),*;
            $(
                $(#[$setter_attr:meta])*
                $setter:ident: $s_ty:ty = $default:expr
            ),*;
        }
    ) => {
        $(#[$builder_attr])*
        pub struct $B<$lifetime, $T: $lifetime, $Cli: $lifetime> {
            $client: $cli_ty,
            $($arg: $a_ty,)*
            $($(#[$setter_attr])* $setter: $s_ty,)*
        }

        impl<$lifetime, C, A> $B<$lifetime, Token<C, A>, ()>
            where C: Borrow<str>, A: Borrow<str>
        {
            /// Create a builder for `POST statuses/filter` endpoint.
            ///
            /// See the [Twitter Developer Documentation][1]
            /// for more information.
            ///
            /// [1]: https://dev.twitter.com/streaming/reference/post/statuses/filter
            pub fn filter(token: &$lifetime Token<C, A>, track: &$lifetime str)
                -> Self
            {
                let mut ret = Self::custom(
                    RequestMethod::POST,
                    Uri::from_shared(Bytes::from_static(
                        b"https://stream.twitter.com/1.1/statuses/filter.json"
                    )).unwrap(),
                    token,
                );
                ret.track(Some(track));
                ret
            }

            /// Create a builder for `GET statuses/sample` endpoint.
            ///
            /// See the [Twitter Developer Documentation][1] for more information.
            ///
            /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/sample
            pub fn sample(token: &$lifetime Token<C, A>) -> Self {
                Self::custom(
                    RequestMethod::GET,
                    Uri::from_shared(Bytes::from_static(
                        b"https://stream.twitter.com/1.1/statuses/sample.json"
                    )).unwrap(),
                    token,
                )
            }

            /// Constructs a builder for a Stream at a custom endpoint.
            pub fn custom(
                method: RequestMethod,
                endpoint: Uri,
                token: &$lifetime Token<C, A>,
            ) -> Self
            {
                $B {
                    $client: $cli_default,
                    method,
                    endpoint,
                    token,
                    $($setter: $default,)*
                }
            }
        }

        impl<$lifetime, C, A, _Cli> $B<$lifetime, Token<C, A>, _Cli> {
            /// Set a `hyper::Client` to be used for connecting to the server.
            ///
            /// The `Client` should be able to handle the `https` scheme.
            pub fn client<Conn, B>(self, client: &$lifetime Client<Conn, B>)
                -> $B<$lifetime, Token<C, A>, Client<Conn, B>>
            where
                Conn: Connect + Sync + 'static,
                Conn::Transport: 'static,
                Conn::Future: 'static,
                B: Default + From<Vec<u8>> + Payload + Send + 'static,
                B::Data: Send,
            {
                $B {
                    $client: client,
                    $($arg: self.$arg,)*
                    $($setter: self.$setter,)*
                }
            }

            /// Unset the client set by `client` method.
            pub fn unset_client(self) -> $B<$lifetime, Token<C, A>, ()> {
                $B {
                    $client: &(),
                    $($arg: self.$arg,)*
                    $($setter: self.$setter,)*
                }
            }

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
            pub fn token(&mut self, token: &$lifetime Token<C, A>) -> &mut Self
            {
                self.token = token;
                self
            }

            $(
                $(#[$setter_attr])*
                pub fn $setter(&mut self, $setter: $s_ty) -> &mut Self {
                    self.$setter = $setter;
                    self
                }
            )*
        }
    };
}

def_builder! {
    /// A builder for `TwitterStream`.
    ///
    /// ## Example
    ///
    /// ```rust,no_run
    /// extern crate twitter_stream;
    ///
    /// use twitter_stream::{Token, TwitterStreamBuilder};
    /// use twitter_stream::rt::{self, Future, Stream};
    ///
    /// # fn main() {
    /// let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");
    ///
    /// let future = TwitterStreamBuilder::sample(&token)
    ///     .timeout(None)
    ///     .listen()
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
    pub struct TwitterStreamBuilder<'a, T, Cli> {
        client: &'a Cli = &();

        method: RequestMethod,
        endpoint: Uri,
        token: &'a T;

        // Setters:

        /// Set a timeout for the stream. `None` means infinity.
        timeout: Option<Duration> = Some(Duration::from_secs(90)),

        // delimited: bool,

        /// Set whether to receive messages when in danger of
        /// being disconnected.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        ///
        /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#stall-warnings
        stall_warnings: bool = false,

        /// Set the minimum `filter_level` Tweet attribute to receive.
        /// The default is `FilterLevel::None`.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        ///
        /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#filter-level
        filter_level: FilterLevel = FilterLevel::None,

        /// Set a comma-separated language identifiers to receive Tweets
        /// written in the specified languages only.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        ///
        /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#language
        language: Option<&'a str> = None,

        /// Set a list of user IDs to receive Tweets only from
        /// the specified users.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        ///
        /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#follow
        follow: Option<&'a [u64]> = None,

        /// A comma separated list of phrases to filter Tweets by.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        ///
        /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#track
        track: Option<&'a str> = None,

        /// Set a list of bounding boxes to filter Tweets by,
        /// specified by a pair of coordinates in the form of
        /// `((longitude, latitude), (longitude, latitude))` tuple.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        ///
        /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#locations
        #[cfg_attr(feature = "cargo-clippy", allow(type_complexity))]
        locations: Option<&'a [((f64, f64), (f64, f64))]> = None,

        /// The `count` parameter.
        /// This parameter requires elevated access to use.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        ///
        /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#count
        count: Option<i32> = None;
    }
}

/// A future returned by constructor methods
/// which resolves to a `TwitterStream`.
pub struct FutureTwitterStream {
    inner: Result<FutureTwitterStreamInner, Option<TlsError>>,
}

/// A listener for Twitter Streaming API.
/// It yields JSON strings returned from the API.
pub struct TwitterStream {
    inner: EitherStream<
        Lines<TimeoutStream<Body>>,
        Lines<Gzip<TimeoutStream<Body>>>,
    >,
}

struct FutureTwitterStreamInner {
    resp: ResponseFuture,
    timeout: Timeout,
}

impl TwitterStream {
    /// A shorthand for `TwitterStreamBuilder::filter(token, track).listen()`.
    pub fn filter<C, A>(token: &Token<C, A>, track: &str) -> FutureTwitterStream
        where C: Borrow<str>, A: Borrow<str>
    {
        TwitterStreamBuilder::filter(token, track).listen()
    }

    /// A shorthand for `TwitterStreamBuilder::sample(token ).listen()`.
    pub fn sample<C, A>(token: &Token<C, A>) -> FutureTwitterStream
        where C: Borrow<str>, A: Borrow<str>
    {
        TwitterStreamBuilder::sample(token).listen()
    }
}

impl<'a, C, A, Conn, B> TwitterStreamBuilder<'a, Token<C, A>, Client<Conn, B>>
where
    C: Borrow<str>,
    A: Borrow<str>,
    Conn: Connect + Sync + 'static,
    Conn::Transport: 'static,
    Conn::Future: 'static,
    B: Default + From<Vec<u8>> + Payload + Send + 'static,
    B::Data: Send,
{
    /// Start listening on a Stream, returning a `Future` which resolves
    /// to a `Stream` yielding JSON messages from the API.
    pub fn listen(&self) -> FutureTwitterStream {
        FutureTwitterStream {
            inner: Ok(FutureTwitterStreamInner {
                resp: self.connect(self.client),
                timeout: self.timeout
                    .map(Timeout::new)
                    .unwrap_or_else(Timeout::never),
            }),
        }
    }
}

impl<'a, C, A> TwitterStreamBuilder<'a, Token<C, A>, ()>
    where C: Borrow<str>, A: Borrow<str>
{
    /// Start listening on a Stream, returning a `Future` which resolves
    /// to a `Stream` yielding JSON messages from the API.
    pub fn listen(&self) -> FutureTwitterStream {
        FutureTwitterStream {
            inner: default_connector::new()
                .map(|c| FutureTwitterStreamInner {
                    resp: self.connect::<_, Body>(&Client::builder().build(c)),
                    timeout: self.timeout
                        .map(Timeout::new)
                        .unwrap_or_else(Timeout::never),
                })
                .map_err(Some),
        }
    }
}

impl<'a, C, A, _Cli> TwitterStreamBuilder<'a, Token<C, A>, _Cli>
    where C: Borrow<str>, A: Borrow<str>
{
    /// Make an HTTP connection to an endpoint of the Streaming API.
    fn connect<Conn, B>(&self, c: &Client<Conn, B>) -> ResponseFuture
    where
        Conn: Connect + Sync + 'static,
        Conn::Transport: 'static,
        Conn::Future: 'static,
        B: Default + From<Vec<u8>> + Payload + Send + 'static,
        B::Data: Send,
    {
        let mut req = Request::builder();
        req.method(self.method.clone())
            .header(ACCEPT_ENCODING, HeaderValue::from_static("chunked,gzip"));

        let req = if RequestMethod::POST == self.method {
            let query = QueryBuilder::new_form(
                self.token.consumer_secret.borrow(),
                self.token.access_secret.borrow(),
                "POST", &self.endpoint,
            );
            let QueryOutcome { header, query } = self.build_query(query);

            req
                .uri(self.endpoint.clone())
                .header(AUTHORIZATION, Bytes::from(header))
                .header(CONTENT_TYPE, HeaderValue::from_static(
                    "application/x-www-form-urlencoded"
                ))
                .header(CONTENT_LENGTH, Bytes::from(query.len().to_string()))
                .body(query.into_bytes().into())
                .unwrap()
        } else {
            let query = QueryBuilder::new(
                self.token.consumer_secret.borrow(),
                self.token.access_secret.borrow(),
                self.method.as_ref(), &self.endpoint,
            );
            let QueryOutcome { header, query: uri } = self.build_query(query);

            req
                .uri(uri)
                .header(AUTHORIZATION, Bytes::from(header))
                .body(B::default())
                .unwrap()
        };

            c.request(req)
        }

    fn build_query(&self, mut query: QueryBuilder) -> QueryOutcome {
        const COMMA: &str = "%2C";
        const COMMA_DOUBLE_ENCODED: &str = "%252C";
        if let Some(n) = self.count {
            query.append_encoded("count", n, n, false);
        }
        if self.filter_level != FilterLevel::None {
            query.append("filter_level", self.filter_level.as_ref(), false);
        }
        if let Some(ids) = self.follow {
            query.append_encoded(
                "follow",
                JoinDisplay(ids, COMMA),
                JoinDisplay(ids, COMMA_DOUBLE_ENCODED),
                false,
            );
        }
        if let Some(s) = self.language {
            query.append("language", s, false);
        }
        if let Some(locs) = self.locations {
            struct LocationsDisplay<'a, D>(&'a [((f64, f64), (f64, f64))], D);
            impl<'a, D: Display> Display for LocationsDisplay<'a, D> {
                fn fmt(&self, f: &mut Formatter) -> fmt::Result {
                    macro_rules! push {
                        ($($c:expr),*) => {{
                            $(write!(f, "{}{}", self.1, $c)?;)*
                        }};
                    }
                    let mut iter = self.0.iter();
                    if let Some(&((x1, y1), (x2, y2))) = iter.next() {
                        write!(f, "{}", x1)?;
                        push!(y1, x2, y2);
                        for &((x1, y1), (x2, y2)) in iter {
                            push!(x1, y1, x2, y2);
                        }
                    }
                    Ok(())
                }
            }
            query.append_encoded(
                "locations",
                LocationsDisplay(locs, COMMA),
                LocationsDisplay(locs, COMMA_DOUBLE_ENCODED),
                false,
            );
        }
        query.append_oauth_params(
            self.token.consumer_key.borrow(),
            self.token.access_key.borrow(),
            ! (self.stall_warnings || self.track.is_some()),
        );
        if self.stall_warnings {
            query.append_encoded("stall_warnings", "true", "true", ! self.track.is_some());
        }
        if let Some(s) = self.track {
            query.append("track", s, true);
        }

        query.build()
    }
}

impl Future for FutureTwitterStream {
    type Item = TwitterStream;
    type Error = Error;

    fn poll(&mut self) -> Poll<TwitterStream, Error> {
        use futures::Async;

        let FutureTwitterStreamInner { ref mut resp, ref mut timeout } =
            *self.inner.as_mut().map_err(|e| Error::Tls(
                e.take().expect("cannot poll FutureTwitterStream twice")
            ))?;

        match resp.poll().map_err(Error::Hyper)? {
            Async::Ready(res) => {
                let (Parts { status, headers, .. }, body) = res.into_parts();

                if StatusCode::OK != status {
                    return Err(Error::Http(status));
                }

                let body = timeout.take().for_stream(body);
                let gzip = headers.get_all(CONTENT_ENCODING)
                    .iter()
                    .any(|e| e == "gzip");
                let inner = if gzip {
                    EitherStream::B(Lines::new(Gzip::new(body)))
                } else {
                    EitherStream::A(Lines::new(body))
                };

                Ok(TwitterStream { inner }.into())
            },
            Async::NotReady => {
                match timeout.poll() {
                    Ok(Async::Ready(())) => Err(Error::TimedOut),
                    Ok(Async::NotReady) => Ok(Async::NotReady),
                    Err(_never) => unreachable!(),
                }
            },
        }
    }
}

impl Stream for TwitterStream {
    type Item = JsonStr;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<JsonStr>, Error> {
        loop {
            match try_ready!(self.inner.poll()) {
                Some(line) => {
                    // Skip whitespaces (as in RFC7159 §2)
                    let all_ws = line.iter().all(|&c| {
                        c == b'\n' || c == b'\r' || c == b' ' || c == b'\t'
                    });
                    if ! all_ws {
                        let line = JsonStr::from_utf8(line)
                            .map_err(Error::Utf8)?;
                        return Ok(Some(line).into());
                    }
                },
                None => return Ok(None.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_dictionary_order() {
        let endpoint = "https://stream.twitter.com/1.1/statuses/filter.json"
            .parse::<Uri>().unwrap();
        TwitterStreamBuilder {
            client: &(),
            method: RequestMethod::GET,
            endpoint: endpoint.clone(),
            token: &Token::new("", "", "", ""),
            timeout: None,
            stall_warnings: true,
            filter_level: FilterLevel::Low,
            language: Some("en"),
            follow: Some(&[12]),
            track: Some("\"User Stream\" to:TwitterDev"),
            locations: Some(&[((37.7748, -122.4146), (37.7788, -122.4186))]),
            count: Some(10),
        }.build_query(QueryBuilder::new_form("", "", "", &endpoint));
        // `QueryBuilder::check_dictionary_order` will panic
        // if the insertion order of query pairs is incorrect.
    }
}
