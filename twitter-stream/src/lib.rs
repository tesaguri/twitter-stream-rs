/*!
# Twitter Stream

A library for listening on Twitter Streaming API.

## Usage

Add `twitter-stream` to your dependencies in your project's `Cargo.toml`:

```toml
[dependencies]
twitter-stream = "0.2"
```

and this to your crate root:

```rust,no_run
extern crate twitter_stream;
```

## Overview

Here is a basic example that prints each Tweet's text from User Stream:

```rust,no_run
extern crate futures;
extern crate tokio_core;
extern crate twitter_stream;

use futures::{Future, Stream};
use tokio_core::reactor::Core;
use twitter_stream::{Token, TwitterStream};
use twitter_stream::message::StreamMessage;

# fn main() {
let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

let mut core = Core::new().unwrap();

let future = TwitterStream::user(&token, &core.handle()).flatten_stream().for_each(|json| {
    if let Ok(StreamMessage::Tweet(tweet)) = twitter_stream::message::parse(&json) {
        println!("{}", tweet.text);
    }
    Ok(())
});

core.run(future).unwrap();
# }
```
*/

extern crate bytes;
#[macro_use]
extern crate futures;
extern crate hyper;
#[cfg(feature = "tls")]
extern crate hyper_tls;
#[macro_use]
extern crate lazy_static;
extern crate oauthcli;
#[cfg(feature = "use-serde")]
extern crate serde;
#[cfg(feature = "use-serde")]
#[macro_use]
extern crate serde_derive;
extern crate tokio_core;
#[cfg(feature = "parse")]
extern crate twitter_stream_message;
extern crate url;

#[macro_use]
mod util;

pub mod error;
#[cfg(feature = "parse")]
pub mod message;
pub mod types;

mod auth;

pub use auth::Token;
pub use error::Error;

use error::HyperError;
use futures::{Future, Poll, Stream};
use hyper::Body;
use hyper::client::{Client, Connect, FutureResponse, Request};
use hyper::header::{Headers, ContentType, UserAgent};
use std::ops::Deref;
use std::borrow::Cow;
use std::time::Duration;
use tokio_core::reactor::Handle;
use types::{FilterLevel, JsonStr, RequestMethod, StatusCode, Url, With};
use url::form_urlencoded::{Serializer, Target};
use util::{BaseTimeout, Lines, TimeoutStream};

macro_rules! def_stream {
    (
        $(#[$builder_attr:meta])*
        pub struct $B:ident<$lifetime:tt, $CH:ident> {
            $client_or_handle:ident: $ch_ty:ty = $ch_default:expr;
            $(
                $(#[$arg_setter_attr:meta])*
                :$arg:ident: $a_ty:ty
            ),*;
            $(
                $(#[$setter_attr:meta])*
                :$setter:ident: $s_ty:ty = $default:expr
            ),*;
            $(:$custom_setter:ident: $c_ty:ty = $c_default:expr),*;
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
            pub fn $constructor:ident($Method:ident, $end_point:expr);
        )*
    ) => {
        $(#[$builder_attr])*
        pub struct $B<$lifetime, $CH: 'a> {
            $client_or_handle: $ch_ty,
            $($arg: $a_ty,)*
            $($setter: $s_ty,)*
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

        impl<$lifetime> $B<$lifetime, ()> {
            $(
                $(#[$constructor_attr])*
                pub fn $constructor(token: &$lifetime Token<$lifetime>) -> $B<$lifetime, ()> {
                    $B::custom(RequestMethod::$Method, $end_point.deref(), token)
                }
            )*

            /// Constructs a builder for a Stream at a custom end point.
            pub fn custom($($arg: $a_ty),*) -> $B<$lifetime, ()> {
                $B {
                    $client_or_handle: $ch_default,
                    $($arg: $arg,)*
                    $($setter: $default,)*
                    $($custom_setter: $c_default,)*
                }
            }
        }

        impl<$lifetime, $CH> $B<$lifetime, $CH> {
            /// Set a `hyper::Client` to be used for connecting to the server.
            ///
            /// The `Client` should be able to handle the `https` scheme.
            ///
            /// This method overrides the effect of `handle` method.
            pub fn client<C, B>(self, client: &$lifetime Client<C, B>) -> $B<$lifetime, Client<C, B>>
                where C: Connect, B: From<Vec<u8>> + Stream<Error=HyperError> + 'static, B::Item: AsRef<[u8]>
            {
                $B {
                    $client_or_handle: client,
                    $($arg: self.$arg,)*
                    $($setter: self.$setter,)*
                    $($custom_setter: self.$custom_setter,)*
                }
            }

            /// Set a `tokio_core::reactor::Handle` to be used for connecting to the server.
            ///
            /// This method overrides the effect of `client` method.
            pub fn handle(self, handle: &$lifetime Handle) -> $B<$lifetime, Handle> {
                $B {
                    $client_or_handle: handle,
                    $($arg: self.$arg,)*
                    $($setter: self.$setter,)*
                    $($custom_setter: self.$custom_setter,)*
                }
            }

            $(
                $(#[$arg_setter_attr])*
                pub fn $arg(&mut self, $arg: $a_ty) -> &mut Self {
                    self.$arg = $arg;
                    self
                }
            )*

            /// Set a user agent string to be sent when connectiong to the Stream.
            pub fn user_agent<T>(&mut self, user_agent: Option<T>) -> &mut Self where T: Into<Cow<'static, str>> {
                self.user_agent = user_agent.map(Into::into);
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

        impl $S {
            $(
                $(#[$s_constructor_attr])*
                pub fn $constructor(token: &Token, handle: &Handle) -> $FS {
                    $B::$constructor(token).handle(handle).listen()
                }
            )*
        }
    };
}

lazy_static! {
    static ref EP_FILTER: Url = Url::parse("https://stream.twitter.com/1.1/statuses/filter.json").unwrap();
    static ref EP_SAMPLE: Url = Url::parse("https://stream.twitter.com/1.1/statuses/sample.json").unwrap();
    static ref EP_USER: Url = Url::parse("https://userstream.twitter.com/1.1/user.json").unwrap();
}

const TUPLE_REF: &'static () = &();

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
    ///     .listen() // You cannot call `listen` method before you call `client` or `handle` method.
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
    pub struct TwitterStreamBuilder<'a, CH> {
        client_or_handle: &'a CH = TUPLE_REF;

        /// Reset the HTTP request method to be used when connecting to the server.
        :method: RequestMethod,

        /// Reset the API endpoint URL to be connected.
        :end_point: &'a Url,

        /// Reset the token to be used to log into Twitter.
        :token: &'a Token<'a>;

        // Setters:

        /// Set a timeout for the stream. `None` means infinity.
        :timeout: Option<Duration> = None,

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

        /// Set a comma-separated language identifiers to receive Tweets written in the specified languages only.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#language
        :language: Option<&'a str> = None,

        /// Set a list of user IDs to receive Tweets only from the specified users.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1] https://dev.twitter.com/streaming/overview/request-parameters#follow
        :follow: Option<&'a [u64]> = None,

        /// A comma separated list of phrases to filter Tweets by.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#track
        :track: Option<&'a str> = None,

        /// Set a list of bounding boxes to filter Tweets by, specified by a pair of coordinates in
        /// the form of ((longitude, latitude), (longitude, latitude)) tuple.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#locations
        :locations: Option<&'a [((f64, f64), (f64, f64))]> = None,

        /// The `count` parameter. This parameter requires elevated access to use.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#count
        :count: Option<i32> = None,

        /// Set types of messages delivered to User and Site Streams clients.
        :with: Option<With> = None,

        /// Set whether to receive all @replies.
        ///
        /// See the [Twitter Developer Documentation][1] for more information.
        /// [1]: https://dev.twitter.com/streaming/overview/request-parameters#replies
        :replies: bool = false;

        // stringify_friend_ids: bool;

        // Fields whose setters are manually defined elsewhere:

        :user_agent: Option<Cow<'static, str>> = None;
    }

    /// A future returned by constructor methods which resolves to a `TwitterStream`.
    pub struct FutureTwitterStream {
        inner: FutureResponse,
        timeout: Option<BaseTimeout>,
    }

    /// A listener for Twitter Streaming API. It yields JSON strings returned from the API.
    pub struct TwitterStream {
        inner: Lines<TimeoutStream<Body>>,
    }

    // Constructors for `TwitterStreamBuilder`:

    /// Create a builder for `POST statuses/filter` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/post/statuses/filter
    -
    /// A shorthand for `TwitterStreamBuilder::filter().listen()`.
    pub fn filter(Post, EP_FILTER);

    /// Create a builder for `GET statuses/sample` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/sample
    -
    /// A shorthand for `TwitterStreamBuilder::sample().listen()`.
    pub fn sample(Get, EP_SAMPLE);

    /// Create a builder for `GET user` endpoint (a.k.a. User Stream).
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/get/user
    -
    /// A shorthand for `TwitterStreamBuilder::user().listen()`.
    pub fn user(Get, EP_USER);
}

impl<'a, C, B> TwitterStreamBuilder<'a, Client<C, B>>
    where C: Connect, B: From<Vec<u8>> + Stream<Error=HyperError> + 'static, B::Item: AsRef<[u8]>
{
     /// Attempt to start listening on a Stream and returns a `Stream` object which yields parsed messages from the API.
     ///
     /// You need to call `client` method before trying to call this method.
    pub fn listen(&self) -> FutureTwitterStream {
        FutureTwitterStream {
            inner: self.connect(self.client_or_handle),
            timeout: self.timeout.and_then(|dur| BaseTimeout::new(dur, self.client_or_handle.handle().clone())),
        }
    }
}

impl<'a> TwitterStreamBuilder<'a, Handle> {
     /// Attempt to start listening on a Stream and returns a `Stream` object which yields JSON messages from the API.
     ///
     /// You need to call `handle` method before trying to call this method.
    pub fn listen(&self) -> FutureTwitterStream {
        FutureTwitterStream {
            inner: self.connect(&default_client(self.client_or_handle)),
            timeout: self.timeout.and_then(|dur| BaseTimeout::new(dur, self.client_or_handle.clone())),
        }
    }
}

impl<'a, _CH> TwitterStreamBuilder<'a, _CH> {
    /// Attempt to make an HTTP connection to an end point of the Streaming API.
    fn connect<C, B>(&self, c: &Client<C, B>) -> FutureResponse
        where C: Connect, B: From<Vec<u8>> + Stream<Error=HyperError> + 'static, B::Item: AsRef<[u8]>
    {
        let mut url = self.end_point.clone();

        let mut headers = Headers::new();
        // headers.set(AcceptEncoding(vec![qitem(Encoding::Chunked), qitem(Encoding::Gzip)]));
        if let Some(ref ua) = self.user_agent {
            headers.set(UserAgent::new(ua.clone()));
        }

        if RequestMethod::Post == self.method {
            use hyper::mime;

            let mut body = Serializer::new(String::new());
            self.append_query_pairs(&mut body);
            let body = body.finish();

            headers.set(auth::create_authorization_header(self.token, &self.method, &url, Some(body.as_ref())));
            headers.set(ContentType(mime::APPLICATION_WWW_FORM_URLENCODED));

            let mut req = Request::new(RequestMethod::Post, url.as_ref().parse().unwrap());
            *req.headers_mut() = headers;
            req.set_body(body.into_bytes());

            c.request(req)
        } else {
            self.append_query_pairs(&mut url.query_pairs_mut());
            headers.set(auth::create_authorization_header(self.token, &self.method, &url, None));

            let mut req = Request::new(self.method.clone(), url.as_ref().parse().unwrap());
            *req.headers_mut() = headers;

            c.request(req)
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
}

impl Future for FutureTwitterStream {
    type Item = TwitterStream;
    type Error = Error;

    fn poll(&mut self) -> Poll<TwitterStream, Error> {
        use futures::Async;

        match self.inner.poll().map_err(Error::Hyper)? {
            Async::Ready(res) => {
                let status = res.status();
                if StatusCode::Ok != status {
                    return Err(Error::Http(status));
                }

                let body = match self.timeout.take() {
                    Some(timeout) => timeout.for_stream(res.body()),
                    None => TimeoutStream::never(res.body()),
                };

                Ok(TwitterStream {
                    inner: Lines::new(body),
                }.into())
            },
            Async::NotReady => {
                if let Some(ref mut timeout) = self.timeout {
                    match timeout.timer_mut().poll() {
                        Ok(Async::Ready(())) => return Err(Error::TimedOut),
                        Ok(Async::NotReady) => (),
                        Err(_) => unreachable!(), // `Timeout` never fails.
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
                     // Skip whitespaces (RFC7159 §2)
                    if ! line.iter().all(|&c| c == b'\n' || c == b'\r' || c == b' ' || c == b'\t') {
                        let line = JsonStr::from_utf8(line).map_err(Error::Utf8)?;
                        return Ok(Some(line).into());
                    }
                },
                None => return Ok(None.into()),
            }
        }
    }
}

#[cfg(feature = "tls")]
fn default_client(h: &Handle) -> Client<hyper_tls::HttpsConnector> {
    Client::configure().connector(hyper_tls::HttpsConnector::new(1, h)).build(h)
}

#[cfg(not(feature = "tls"))]
#[cold]
fn default_client(h: &Handle) -> Client<hyper::client::HttpConnector> {
    Client::new(h)
}
