/*!
# Twitter Stream

A library for listening on Twitter Streaming API.

## Usage

Add `twitter-stream` to your dependencies in your project's `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.5"
```

and this to your crate root:

```rust,no_run
extern crate twitter_stream;
```

## Overview

Here is a basic example that prints public mentions @Twitter in JSON format:

```rust,no_run
extern crate futures;
extern crate tokio_core;
extern crate twitter_stream;

use futures::{Future, Stream};
use tokio_core::reactor::Core;
use twitter_stream::{Token, TwitterStreamBuilder};

# fn main() {
let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

let mut core = Core::new().unwrap();

let future = TwitterStreamBuilder::filter(&token).handle(&core.handle())
    .replies(true)
    .track(Some("@Twitter"))
    .listen()
    .flatten_stream()
    .for_each(|json| {
        println!("{}", json);
        Ok(())
    });

core.run(future).unwrap();
# }
```
*/

extern crate bytes;
#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate futures;
extern crate hmac;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate byteorder;
extern crate percent_encoding;
extern crate rand;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
extern crate sha1;
extern crate tokio_core;
#[cfg(feature = "parse")]
extern crate twitter_stream_message;

#[macro_use]
mod util;

pub mod error;
pub mod types;

/// Exports `twitter_stream_message` crate for convenience.
/// This module requires `parse` feature flag to be enabled.
#[cfg(feature = "parse")]
#[deprecated(
    since = "0.6.0",
    note = "use `extern crate twitter_stream_message;` instead",
)]
pub mod message {
    pub use twitter_stream_message::*;
}

mod query_builder;
mod token;

pub use token::Token;
pub use error::Error;

use std::borrow::{Borrow, Cow};
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use futures::{Future, Poll, Stream};
use hyper::Body;
use hyper::client::{Client, Connect, FutureResponse, Request};
use hyper::header::{Headers, ContentLength, ContentType, UserAgent};
use tokio_core::reactor::Handle;

use error::{HyperError, TlsError};
use query_builder::{QueryBuilder, QueryOutcome};
use types::{FilterLevel, JsonStr, RequestMethod, StatusCode, Uri, With};
use util::{BaseTimeout, JoinDisplay, Lines, TimeoutStream};

macro_rules! def_stream {
    (
        $(#[$builder_attr:meta])*
        pub struct $B:ident<$lifetime:tt, $T:ident, $CH:ident> {
            $client_or_handle:ident: $ch_ty:ty = $ch_default:expr;
            $($arg:ident: $a_ty:ty),*;
            $(
                $(#[$setter_attr:meta])*
                $setter:ident: $s_ty:ty = $default:expr
            ),*;
            $($custom_setter:ident: $c_ty:ty = $c_default:expr),*;
        }

        $(#[$future_stream_attr:meta])*
        pub struct $FS:ident {
            $($fs_field:ident: $fsf_ty:ty,)*
        }

        $(#[$stream_attr:meta])*
        pub struct $S:ident {
            $($s_field:ident: $sf_ty:ty,)*
        }

        $(
            $(#[$constructor_attr:meta])*
            -
            $(#[$s_constructor_attr:meta])*
            pub fn $constructor:ident($Method:ident, $endpoint:expr);
        )*
    ) => {
        $(#[$builder_attr])*
        pub struct $B<$lifetime, $T: $lifetime, $CH: $lifetime> {
            $client_or_handle: $ch_ty,
            $($arg: $a_ty,)*
            $($(#[$setter_attr])* $setter: $s_ty,)*
            $($custom_setter: $c_ty,)*
        }

        $(#[$future_stream_attr])*
        pub struct $FS {
            $($fs_field: $fsf_ty,)*
        }

        $(#[$stream_attr])*
        pub struct $S {
            $($s_field: $sf_ty,)*
        }

        impl<$lifetime, C, A> $B<$lifetime, Token<C, A>, ()>
            where C: Borrow<str>, A: Borrow<str>
        {
            $(
                $(#[$constructor_attr])*
                pub fn $constructor(token: &$lifetime Token<C, A>) -> Self {
                    $B::custom(
                        RequestMethod::$Method,
                        &*$endpoint,
                        token,
                    )
                }
            )*

            /// Constructs a builder for a Stream at a custom endpoint.
            pub fn custom(
                method: RequestMethod,
                endpoint: &$lifetime Uri,
                token: &$lifetime Token<C, A>,
            ) -> Self
            {
                $B {
                    $client_or_handle: $ch_default,
                    method,
                    endpoint,
                    token,
                    $($setter: $default,)*
                    $($custom_setter: $c_default,)*
                }
            }
        }

        impl<$lifetime, C, A, _CH> $B<$lifetime, Token<C, A>, _CH> {
            /// Set a `hyper::Client` to be used for connecting to the server.
            ///
            /// The `Client` should be able to handle the `https` scheme.
            ///
            /// This method overrides the effect of `handle` method.
            pub fn client<Conn, B>(self, client: &$lifetime Client<Conn, B>)
                -> $B<$lifetime, Token<C, A>, Client<Conn, B>>
            where
                Conn: Connect,
                B: From<Vec<u8>> + Stream<Error=HyperError> + 'static,
                B::Item: AsRef<[u8]>,
            {
                $B {
                    $client_or_handle: client,
                    $($arg: self.$arg,)*
                    $($setter: self.$setter,)*
                    $($custom_setter: self.$custom_setter,)*
                }
            }

            /// Set a `tokio_core::reactor::Handle` to be used for
            /// connecting to the server.
            ///
            /// This method overrides the effect of `client` method.
            pub fn handle(self, handle: &$lifetime Handle)
                -> $B<$lifetime, Token<C, A>, Handle>
            {
                $B {
                    $client_or_handle: handle,
                    $($arg: self.$arg,)*
                    $($setter: self.$setter,)*
                    $($custom_setter: self.$custom_setter,)*
                }
            }

            /// Reset the HTTP request method to be used when connecting
            /// to the server.
            pub fn method(&mut self, method: RequestMethod) -> &mut Self {
                self.method = method;
                self
            }

            /// Reset the API endpoint URI to be connected.
            pub fn endpoint(&mut self, endpoint: &$lifetime Uri) -> &mut Self {
                self.endpoint = endpoint;
                self
            }

            /// Reset the API endpoint URI to be connected.
            #[deprecated(since = "0.6.0", note = "Use `endpoint` instead")]
            pub fn end_point(&mut self, end_point: &$lifetime Uri) -> &mut Self
            {
                self.endpoint = end_point;
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

            /// Set a user agent string to be sent when connectiong to
            /// the Stream.
            #[deprecated(since = "0.6.0", note = "Will be removed in 0.7")]
            pub fn user_agent<U>(&mut self, user_agent: Option<U>) -> &mut Self
                where U: Into<Cow<'static, str>>
            {
                self.user_agent = user_agent.map(Into::into);
                self
            }
        }

        impl $S {
            $(
                $(#[$s_constructor_attr])*
                #[allow(deprecated)]
                pub fn $constructor<C, A>(token: &Token<C, A>, handle: &Handle)
                    -> $FS
                    where C: Borrow<str>, A: Borrow<str>
                {
                    $B::$constructor(token).handle(handle).listen()
                }
            )*
        }
    };
}

lazy_static! {
    static ref EP_FILTER: Uri =
        "https://stream.twitter.com/1.1/statuses/filter.json".parse().unwrap();
    static ref EP_SAMPLE: Uri =
        "https://stream.twitter.com/1.1/statuses/sample.json".parse().unwrap();
    static ref EP_USER: Uri =
        "https://userstream.twitter.com/1.1/user.json".parse().unwrap();
}

const TUPLE_REF: &() = &();

def_stream! {
    /// A builder for `TwitterStream`.
    ///
    /// ## Example
    ///
    /// ```rust,no_run
    /// extern crate futures;
    /// extern crate tokio_core;
    /// extern crate twitter_stream;
    ///
    /// use futures::{Future, Stream};
    /// use tokio_core::reactor::Core;
    /// use twitter_stream::{Token, TwitterStreamBuilder};
    ///
    /// # fn main() {
    /// let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");
    /// let mut core = Core::new().unwrap();
    /// let handle = core.handle();
    ///
    /// let future = TwitterStreamBuilder::user(&token)
    ///     .handle(&handle)
    ///     .timeout(None)
    ///     .replies(true)
    ///     .listen() // You cannot use `listen` method before calling `client` or `handle` method.
    ///     .flatten_stream()
    ///     .for_each(|json| {
    ///         println!("{}", json);
    ///         Ok(())
    ///     });
    ///
    /// core.run(future).unwrap();
    /// # }
    /// ```
    #[derive(Clone, Debug)]
    pub struct TwitterStreamBuilder<'a, T, CH> {
        client_or_handle: &'a CH = TUPLE_REF;

        method: RequestMethod,
        endpoint: &'a Uri,
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
        count: Option<i32> = None,

        /// Set types of messages delivered to User and Site Streams clients.
        with: Option<With> = None,

        /// Set whether to receive all @replies.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        ///
        /// [1]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/basic-stream-parameters#replies
        replies: bool = false;

        // stringify_friend_ids: bool;

        // Fields whose setters are manually defined elsewhere:

        user_agent: Option<Cow<'static, str>> = None;
    }

    /// A future returned by constructor methods
    /// which resolves to a `TwitterStream`.
    pub struct FutureTwitterStream {
        inner: Result<FutureTwitterStreamInner, Option<TlsError>>,
    }

    /// A listener for Twitter Streaming API.
    /// It yields JSON strings returned from the API.
    pub struct TwitterStream {
        inner: Lines<TimeoutStream<Body>>,
    }

    // Constructors for `TwitterStreamBuilder`:

    /// Create a builder for `POST statuses/filter` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/post/statuses/filter
    -
    /// A shorthand for `TwitterStreamBuilder::filter().listen()`.
    pub fn filter(Post, EP_FILTER);

    /// Create a builder for `GET statuses/sample` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/sample
    -
    /// A shorthand for `TwitterStreamBuilder::sample().listen()`.
    pub fn sample(Get, EP_SAMPLE);

    /// Create a builder for `GET user` endpoint (a.k.a. User Stream).
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/get/user
    #[deprecated(
        since = "0.6.0",
        note = "The User stream has been deprecated and will be unavailable",
    )]
    -
    /// A shorthand for `TwitterStreamBuilder::user().listen()`.
    #[deprecated(
        since = "0.6.0",
        note = "The User stream has been deprecated and will be unavailable",
    )]
    pub fn user(Get, EP_USER);
}

struct FutureTwitterStreamInner {
    resp: FutureResponse,
    timeout: Option<BaseTimeout>,
}

impl<'a, C, A, Conn, B> TwitterStreamBuilder<'a, Token<C, A>, Client<Conn, B>>
where
    C: Borrow<str>,
    A: Borrow<str>,
    Conn: Connect,
    B: From<Vec<u8>> + Stream<Error=HyperError> + 'static,
    B::Item: AsRef<[u8]>,
{
    /// Start listening on a Stream, returning a `Future` which resolves
    /// to a `Stream` yielding JSON messages from the API.
    ///
    /// You need to call `handle` method before calling this method.
    #[allow(deprecated)]
    pub fn listen(&self) -> FutureTwitterStream {
        FutureTwitterStream {
            inner: Ok(FutureTwitterStreamInner {
                resp: self.connect(self.client_or_handle),
                timeout: self.timeout.and_then(|dur| {
                    let h = self.client_or_handle.handle().clone();
                    BaseTimeout::new(dur, h)
                }),
            }),
        }
    }
}

impl<'a, C, A> TwitterStreamBuilder<'a, Token<C, A>, Handle>
    where C: Borrow<str>, A: Borrow<str>
{
    /// Start listening on a Stream, returning a `Future` which resolves
    /// to a `Stream` yielding JSON messages from the API.
    ///
    /// You need to call `handle` method before calling this method.
    pub fn listen(&self) -> FutureTwitterStream {
        FutureTwitterStream {
            inner: default_connector::new(self.client_or_handle)
                .map(|c| FutureTwitterStreamInner {
                    resp: self.connect(
                        &Client::configure().connector(c)
                            .build(self.client_or_handle)
                    ),
                    timeout: self.timeout.and_then(|dur| {
                        let h = self.client_or_handle.clone();
                        BaseTimeout::new(dur, h)
                    }),
                })
                .map_err(Some),
        }
    }
}

impl<'a, C, A, _CH> TwitterStreamBuilder<'a, Token<C, A>, _CH>
    where C: Borrow<str>, A: Borrow<str>
{
    /// Make an HTTP connection to an endpoint of the Streaming API.
    fn connect<Conn, B>(&self, c: &Client<Conn, B>) -> FutureResponse
    where
        Conn: Connect,
        B: From<Vec<u8>> + Stream<Error=HyperError> + 'static,
        B::Item: AsRef<[u8]>,
    {
        let mut headers = Headers::new();
        // headers.set(AcceptEncoding(vec![qitem(Encoding::Chunked), qitem(Encoding::Gzip)]));
        if let Some(ref ua) = self.user_agent {
            headers.set(UserAgent::new(ua.clone()));
        }

        if RequestMethod::Post == self.method {
            use hyper::mime;

            let query = QueryBuilder::new_form(
                self.token.consumer_secret.borrow(),
                self.token.access_secret.borrow(),
                "POST", self.endpoint.as_ref(),
            );
            let QueryOutcome { header, query } = self.build_query(query);

            headers.set_raw("Authorization", header);
            headers.set(ContentType(mime::APPLICATION_WWW_FORM_URLENCODED));
            headers.set(ContentLength(query.len() as u64));

            let mut req = Request::new(
                RequestMethod::Post,
                self.endpoint.clone(),
            );
            *req.headers_mut() = headers;
            req.set_body(query.into_bytes());

            c.request(req)
        } else {
            let upcase;
            let query = QueryBuilder::new(
                self.token.consumer_secret.borrow(),
                self.token.access_secret.borrow(),
                match self.method {
                    RequestMethod::Extension(ref m) => {
                        upcase = m.to_ascii_uppercase();
                        &upcase
                    },
                    ref m => m.as_ref(),
                },
                self.endpoint.as_ref().to_owned(),
            );
            let QueryOutcome { header, query: uri } = self.build_query(query);

            headers.set_raw("Authorization", header);

            let mut req = Request::new(
                self.method.clone(),
                uri.parse().unwrap(),
            );
            *req.headers_mut() = headers;

            c.request(req)
        }
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
            ! (self.replies || self.stall_warnings
                || self.track.is_some() || self.with.is_some())
        );
        if self.replies {
            query.append_encoded("replies", "all", "all",
                ! (self.stall_warnings
                    || self.track.is_some() || self.with.is_some())
            );
        }
        if self.stall_warnings {
            query.append_encoded("stall_warnings", "true", "true",
                ! (self.track.is_some() || self.with.is_some())
            );
        }
        if let Some(s) = self.track {
            query.append("track", s, ! self.with.is_some());
        }
        if let Some(ref w) = self.with {
            query.append("with", w.as_ref(), true);
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
                let status = res.status();
                if StatusCode::Ok != status {
                    return Err(Error::Http(status));
                }

                let body = match timeout.take() {
                    Some(timeout) => timeout.for_stream(res.body()),
                    None => TimeoutStream::never(res.body()),
                };

                Ok(TwitterStream { inner: Lines::new(body) }.into())
            },
            Async::NotReady => {
                if let Some(ref mut timeout) = *timeout {
                    match timeout.timer_mut().poll() {
                        Ok(Async::Ready(())) =>
                            return Err(Error::TimedOut),
                        Ok(Async::NotReady) => (),
                        // `Timeout` never fails.
                        Err(_) => unreachable!(),
                    }
                }
                Ok(Async::NotReady)
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

cfg_if! {
    if #[cfg(feature = "tls")] {
        mod default_connector {
            extern crate hyper_tls;
            extern crate native_tls;

            pub use self::native_tls::Error as Error;

            use self::hyper_tls::HttpsConnector;

            pub fn new(h: &::tokio_core::reactor::Handle)
                -> Result<HttpsConnector<::hyper::client::HttpConnector>, Error>
            {
                HttpsConnector::new(1, h)
            }
        }
    } else if #[cfg(feature = "tls-rustls")] {
        mod default_connector {
            extern crate hyper_rustls;

            pub use util::Never as Error;

            use self::hyper_rustls::HttpsConnector;

            pub fn new(h: &::tokio_core::reactor::Handle) -> Result<HttpsConnector, Error> {
                Ok(HttpsConnector::new(1, h))
            }
        }
    } else if #[cfg(feature = "tls-openssl")] {
        mod default_connector {
            extern crate hyper_openssl;

            pub use self::hyper_openssl::openssl::error::ErrorStack as Error;

            use self::hyper_openssl::HttpsConnector;

            pub fn new(h: &::tokio_core::reactor::Handle) -> Result<HttpsConnector<::hyper::client::HttpConnector>, Error> {
                HttpsConnector::new(1, h)
            }
        }
    } else {
        mod default_connector {
            pub use util::Never as Error;

            use hyper::client::HttpConnector;

            #[cold]
            pub fn new(h: &::tokio_core::reactor::Handle) -> Result<HttpConnector, Error> {
                Ok(HttpConnector::new(1, h))
            }
        }
    }
}
