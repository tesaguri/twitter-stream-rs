/*!
# Twitter Stream

A library for listening on Twitter Streaming API.

## Usage

Add `twitter-stream` to your dependencies in your project's `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.8"
```

and this to your crate root:

```rust,no_run
extern crate twitter_stream;
```

## Overview

Here is a basic example that prints public mentions to @Twitter in JSON format:

```rust,no_run
extern crate twitter_stream;

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

extern crate bytes;
extern crate cfg_if;
extern crate futures;
extern crate http;
extern crate hyper;
#[cfg(feature = "tls")]
extern crate hyper_tls;
extern crate libflate;
extern crate oauth1_request as oauth;
#[cfg(feature = "serde")]
extern crate serde;
extern crate string;
extern crate tokio_timer;

#[macro_use]
mod util;

pub mod error;
#[cfg(feature = "runtime")]
pub mod rt;
pub mod types;

mod gzip;
mod token;

pub use error::Error;
pub use token::Token;

use std::borrow::Borrow;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use bytes::Bytes;
use futures::{try_ready, Future, Poll, Stream};
use http::response::Parts;
use hyper::body::{Body, Payload};
use hyper::client::connect::Connect;
use hyper::client::{Client, ResponseFuture};
use hyper::header::{
    HeaderValue, ACCEPT_ENCODING, AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE,
};
use hyper::Request;
use string::TryFrom;

use gzip::MaybeGzip;
use types::{FilterLevel, RequestMethod, StatusCode, Uri};
use util::*;

macro_rules! def_stream {
    (
        $(#[$builder_attr:meta])*
        pub struct $B:ident<$lifetime:tt, $T:ident $(=$TDefault:ty)*> {
            $($arg:ident: $a_ty:ty),*;
            $($setters:tt)*
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
        pub struct $B<$lifetime, $T $(= $TDefault)*> {
            $($arg: $a_ty,)*
            inner: BuilderInner<$lifetime>,
        }

        def_builder_inner! {
            $(#[$builder_attr])*
            struct BuilderInner<$lifetime> { $($setters)* }
        }

        $(#[$future_stream_attr])*
        pub struct $FS {
            $($fs_field: $fsf_ty,)*
        }

        $(#[$stream_attr])*
        pub struct $S {
            $($s_field: $sf_ty,)*
        }

        impl<$lifetime, C, A> $B<$lifetime, Token<C, A>>
        where
            C: Borrow<str>,
            A: Borrow<str>,
        {
            $(
                $(#[$constructor_attr])*
                pub fn $constructor(token: Token<C, A>) -> Self {
                    $B::custom(RequestMethod::$Method, Uri::from_static($endpoint), token)
                }
            )*

            /// Constructs a builder for a Stream at a custom endpoint.
            pub fn custom(
                method: RequestMethod,
                endpoint: Uri,
                token: Token<C, A>,
            ) -> Self
            {
                $B {
                    method,
                    endpoint,
                    token,
                    inner: BuilderInner::new(),
                }
            }

            /// Start listening on the Streaming API endpoint, returning a `Future` which resolves
            /// to a `Stream` yielding JSON messages from the API.
            #[cfg(feature = "tls")]
            pub fn listen(&self) -> Result<$FS, error::TlsError> {
                let conn = hyper_tls::HttpsConnector::new(1)?;
                Ok(self.listen_with_client(&Client::builder().build::<_, Body>(conn)))
            }

            /// Same as `listen` except that it uses `client` to make HTTP request to the endpoint.
            pub fn listen_with_client<Conn, B>(&self, client: &Client<Conn, B>) -> $FS
            where
                Conn: Connect + Sync + 'static,
                Conn::Transport: 'static,
                Conn::Future: 'static,
                B: Default + From<Vec<u8>> + Payload + Send + 'static,
                B::Data: Send,
            {
                self.listen_with_client_(client)
            }
        }

        impl<$lifetime, C, A> $B<$lifetime, Token<C, A>> {
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

            def_setters! { $($setters)* }
        }

        #[cfg(feature = "tls")]
        impl $S {
            $(
                $(#[$s_constructor_attr])*
                pub fn $constructor<C, A>(token: Token<C, A>) -> Result<$FS, error::TlsError>
                where
                    C: Borrow<str>,
                    A: Borrow<str>,
                {
                    $B::$constructor(token).listen()
                }
            )*
        }
    };
}

macro_rules! def_builder_inner {
    (
        $(#[$attr:meta])*
        struct $BI:ident<$lifetime:tt> {
            $($(#[$field_attr:meta])* $field:ident: $t:ty = $default:expr,)*
        }
    ) => {
        $(#[$attr])*
        struct $BI<$lifetime> { $($(#[$field_attr])* $field: $t),* }
        impl<'a> $BI<'a> { fn new() -> Self { $BI { $($field: $default),* } } }
    }
}

macro_rules! def_setters {
    (
        $(#[$attr:meta])* $setter:ident: Option<$t:ty> = $_default:expr,
        $($rest:tt)*
    ) => {
        $(#[$attr])*
        pub fn $setter(&mut self, $setter: impl Into<Option<$t>>) -> &mut Self {
            self.inner.$setter = $setter.into();
            self
        }
        def_setters! { $($rest)* }
    };
    ($(#[$attr:meta])* $setter:ident: $t:ty = $_default:expr, $($rest:tt)*) => {
        $(#[$attr])*
        pub fn $setter(&mut self, $setter: $t) -> &mut Self {
            self.inner.$setter = $setter;
            self
        }
        def_setters! { $($rest)* }
    };
    () => {};
}

def_stream! {
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
        token: T;

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
        filter_level: Option<FilterLevel> = None,

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

    // Constructors for `TwitterStreamBuilder`:

    /// Create a builder for `POST statuses/filter` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/post/statuses/filter
    -
    /// A shorthand for `TwitterStreamBuilder::filter().listen()`.
    pub fn filter(POST, "https://stream.twitter.com/1.1/statuses/filter.json");

    /// Create a builder for `GET statuses/sample` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    ///
    /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/sample
    -
    /// A shorthand for `TwitterStreamBuilder::sample().listen()`.
    pub fn sample(GET, "https://stream.twitter.com/1.1/statuses/sample.json");
}

impl<'a, C, A> TwitterStreamBuilder<'a, Token<C, A>>
where
    C: Borrow<str>,
    A: Borrow<str>,
{
    fn listen_with_client_<Conn, B>(&self, c: &Client<Conn, B>) -> FutureTwitterStream
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
            let signer = oauth::HmacSha1Signer::new_form(
                "POST",
                &self.endpoint,
                self.token.consumer_secret.borrow(),
                self.token.access_secret.borrow(),
            );
            let oauth::Request {
                authorization,
                data,
            } = self.build_query(signer);

            req.uri(self.endpoint.clone())
                .header(AUTHORIZATION, Bytes::from(authorization))
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/x-www-form-urlencoded"),
                ).header(CONTENT_LENGTH, Bytes::from(data.len().to_string()))
                .body(data.into_bytes().into())
                .unwrap()
        } else {
            let signer = oauth::HmacSha1Signer::new(
                self.method.as_ref(),
                &self.endpoint,
                self.token.consumer_secret.borrow(),
                self.token.access_secret.borrow(),
            );
            let oauth::Request {
                authorization,
                data: uri,
            } = self.build_query(signer);

            req.uri(uri)
                .header(AUTHORIZATION, Bytes::from(authorization))
                .body(B::default())
                .unwrap()
        };

        let res = c.request(req);
        FutureTwitterStream {
            response: timeout(res, self.inner.timeout),
        }
    }

    fn build_query(&self, mut signer: oauth::HmacSha1Signer) -> oauth::Request {
        const COMMA: &str = "%2C";
        let this = &self.inner;
        if let Some(n) = this.count {
            signer.parameter_encoded("count", n);
        }
        if let Some(ref fl) = this.filter_level {
            signer.parameter("filter_level", fl.as_ref());
        }
        if let Some(ids) = this.follow {
            signer.parameter_encoded("follow", JoinDisplay(ids, COMMA));
        }
        if let Some(s) = this.language {
            signer.parameter("language", s);
        }
        if let Some(locs) = this.locations {
            struct LocationsDisplay<'a, D>(&'a [((f64, f64), (f64, f64))], D);
            impl<'a, D: Display> Display for LocationsDisplay<'a, D> {
                fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                    macro_rules! push {
                        ($($c:expr),*) => {{ $(write!(f, "{}{}", self.1, $c)?;)* }};
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
            signer.parameter_encoded("locations", LocationsDisplay(locs, COMMA));
        }
        let mut signer = signer.oauth_parameters(
            self.token.consumer_key.borrow(),
            &*oauth::Options::new().token(self.token.access_key.borrow()),
        );
        if this.stall_warnings {
            signer.parameter_encoded("stall_warnings", "true");
        }
        if let Some(s) = this.track {
            signer.parameter("track", s);
        }

        signer.finish()
    }
}

impl Future for FutureTwitterStream {
    type Item = TwitterStream;
    type Error = Error;

    fn poll(&mut self) -> Poll<TwitterStream, Error> {
        let res = try_ready!(self.response.poll());
        let (
            Parts {
                status, headers, ..
            },
            body,
        ) = res.into_parts();

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
            match try_ready!(self.inner.poll()) {
                Some(line) => {
                    // Skip whitespaces (as in RFC7159 §2)
                    let all_ws = line
                        .iter()
                        .all(|&c| c == b'\n' || c == b'\r' || c == b' ' || c == b'\t');
                    if !all_ws {
                        let line = string::String::<Bytes>::try_from(line).map_err(Error::Utf8)?;
                        return Ok(Some(line).into());
                    }
                }
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
            .parse::<Uri>()
            .unwrap();
        TwitterStreamBuilder {
            method: RequestMethod::GET,
            endpoint: endpoint.clone(),
            token: Token::new("", "", "", ""),
            inner: BuilderInner {
                timeout: None,
                stall_warnings: true,
                filter_level: Some(FilterLevel::Low),
                language: Some("en"),
                follow: Some(&[12]),
                track: Some("\"User Stream\" to:TwitterDev"),
                locations: Some(&[((37.7748, -122.4146), (37.7788, -122.4186))]),
                count: Some(10),
            },
        }.build_query(oauth::Signer::new_form("", &endpoint, "", ""));
        // `QueryBuilder::check_dictionary_order` will panic
        // if the insertion order of query pairs is incorrect.
    }
}
