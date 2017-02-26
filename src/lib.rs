/*!
# Twitter Stream

A library for listening on Twitter Streaming API.

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
use twitter_stream::{StreamMessage, Token, TwitterStream};

# fn main() {
let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

let stream = TwitterStream::user(&token).unwrap();

stream
    .for_each(|msg| {
        if let StreamMessage::Tweet(tweet) = msg {
            println!("{}", tweet.text);
        }
        Ok(())
    })
    .wait().unwrap();
# }
```

In the example above, `stream` disconnects and returns an error when a JSON message from Stream has failed to parse.
If you don't want this behavior, you can opt to parse the messages manually:

```rust,no_run
# extern crate futures;
# extern crate twitter_stream;
extern crate serde_json;

# use futures::{Future, Stream};
use twitter_stream::{StreamMessage, Token, TwitterJsonStream};

# fn main() {
# let token = Token::new("", "", "", "");
let stream = TwitterJsonStream::user(&token).unwrap();

stream
    .for_each(|json| {
        if let Ok(StreamMessage::Tweet(tweet)) = serde_json::from_str(&json) {
            println!("{}", tweet.text);
        }
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
pub mod error;
pub mod geometry;
pub mod list;
pub mod message;
pub mod place;
pub mod tweet;
pub mod types;
pub mod user;

mod auth;

pub use auth::Token;
pub use direct_message::DirectMessage;
pub use entities::Entities;
pub use error::{Error, StreamError, Result};
pub use geometry::Geometry;
pub use list::List;
pub use message::StreamMessage;
pub use place::Place;
pub use tweet::Tweet;
pub use user::User;

use futures::{Async, Poll, Stream};
use hyper::client::Client;
use hyper::header::{Headers, AcceptEncoding, ContentEncoding, ContentType, Encoding, UserAgent, qitem};
use std::io::{self, BufReader};
use types::{FilterLevel, RequestMethod, StatusCode, With};
use url::Url;
use url::form_urlencoded::{Serializer, Target};
use util::Lines;
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
            $(
                $(#[$constructor_attr])*
                pub fn $constructor(token: &$lifetime Token<$lifetime>) -> Self
                {
                    $B::custom(RequestMethod::$Method, $end_point, token)
                }
            )*

            /// Constructs a builder for a Stream at a custom end point.
            pub fn custom($($b_field: $bf_ty),*) -> Self {
                $B {
                    $($b_field: $b_field,)*
                    $($setter: $default,)*
                    $($option: None,)*
                }
            }

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
                pub fn $constructor<'a>(token: &'a Token<'a>) -> Result<Self>
                {
                    $B::$constructor(token).listen()
                }
            )*
        }

        impl $JS {
            $(
                $(#[$js_constructor_attr])*
                pub fn $constructor<'a>(token: &'a Token<'a>) -> Result<Self>
                {
                    $B::$constructor(token).listen_json()
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
        token: &'a Token<'a>;

        // Setters:

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

        /// Set a comma-separated language identifiers to receive Tweets written in the specified languages only.
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

    /// A listener for Twitter Streaming API.
    pub struct TwitterStream {
        inner: TwitterJsonStream,
    }

    /// Same as `TwitterStream` except that it yields raw JSON string messages.
    pub struct TwitterJsonStream {
        lines: Lines,
    }

    // Constructors for `TwitterStreamBuilder`:

    /// Create a builder for `POST statuses/filter` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/post/statuses/filter
    -
    /// A shorthand for `TwitterStreamBuilder::filter().listen()`.
    -
    /// A shorthand for `TwitterStreamBuilder::filter().listen_json()`.
    pub fn filter(Post, "https://stream.twitter.com/1.1/statuses/filter.json");

    /// Create a builder for `GET statuses/sample` endpoint.
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/get/statuses/sample
    -
    /// A shorthand for `TwitterStreamBuilder::sample().listen()`.
    -
    /// A shorthand for `TwitterStreamBuilder::sample().listen_json()`.
    pub fn sample(Get, "https://stream.twitter.com/1.1/statuses/sample.json");

    /// Create a builder for `GET user` endpoint (a.k.a. User Stream).
    ///
    /// See the [Twitter Developer Documentation][1] for more information.
    /// [1]: https://dev.twitter.com/streaming/reference/get/user
    -
    /// A shorthand for `TwitterStreamBuilder::user().listen()`.
    -
    /// A shorthand for `TwitterStreamBuilder::user().listen_json()`.
    pub fn user(Get, "https://userstream.twitter.com/1.1/user.json");
}

impl<'a> TwitterStreamBuilder<'a> {
    /// Attempt to start listening on a Stream and returns a `Stream` object which yields parsed messages from the API.
    pub fn listen(&self) -> Result<TwitterStream> {
        Ok(TwitterStream {
            inner: self.listen_json()?,
        })
    }

    /// Attempt to start listening on a Stream and returns a `Stream` which yields JSON messages from the API.
    pub fn listen_json(&self) -> Result<TwitterJsonStream> {
        Ok(TwitterJsonStream {
            lines: self.connect()?,
        })
    }

    /// Attempt to make an HTTP connection to an end point of the Streaming API.
    fn connect(&self) -> Result<Lines> {
        let mut url = Url::parse(self.end_point)?;

        let mut headers = Headers::new();
        headers.set(AcceptEncoding(vec![qitem(Encoding::Chunked), qitem(Encoding::Gzip)]));
        if let Some(ua) = self.user_agent {
            headers.set(UserAgent(ua.to_owned()));
        }

        /// Holds a borrowed or owned value.
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
            let mut cli = default_client::new()?;
            cli.set_read_timeout(Some(std::time::Duration::from_secs(90)));
            Hold::Owned(cli)
        };

        let res = if RequestMethod::Post == self.method {
            use hyper::mime::{Mime, SubLevel, TopLevel};

            headers.set(ContentType(Mime(TopLevel::Application, SubLevel::WwwFormUrlEncoded, Vec::new())));
            let mut body = Serializer::new(String::new());
            self.append_query_pairs(&mut body);
            let body = body.finish();
            headers.set(auth::create_authorization_header(self.token, &self.method, &url, Some(body.as_ref())));
            client
                .post(url)
                .headers(headers)
                .body(&body)
                .send()?
        } else {
            self.append_query_pairs(&mut url.query_pairs_mut());
            headers.set(auth::create_authorization_header(self.token, &self.method, &url, None));
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
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<String>, io::Error> {
        use Async::*;

        loop {
            match self.lines.poll()? {
                Ready(Some(line)) => {
                    if !line.is_empty() {
                        return Ok(Ready(Some(line)));
                    }
                },
                Ready(None) => return Ok(Ready(None)),
                NotReady => return Ok(NotReady),
            }
        }
    }
}

impl IntoIterator for TwitterStream {
    type Item = std::result::Result<StreamMessage, StreamError>;
    type IntoIter = futures::stream::Wait<Self>;

    fn into_iter(self) -> Self::IntoIter {
        self.wait()
    }
}

impl IntoIterator for TwitterJsonStream {
    type Item = io::Result<String>;
    type IntoIter = futures::stream::Wait<Self>;

    fn into_iter(self) -> Self::IntoIter {
        self.wait()
    }
}

#[cfg(feature = "hyper-native-tls")]
mod default_client {
    extern crate hyper_native_tls;
    extern crate native_tls;

    pub use self::native_tls::Error as Error;

    use hyper::{self, Client};

    pub fn new() -> Result<Client, Error> {
        hyper_native_tls::NativeTlsClient::new()
            .map(hyper::net::HttpsConnector::new)
            .map(Client::with_connector)
    }
}

#[cfg(feature = "hyper-openssl")]
mod default_client {
    extern crate hyper_openssl;
    extern crate openssl;

    pub use self::openssl::error::ErrorStack as Error;

    use hyper::{self, Client};

    pub fn new() -> Result<Client, Error> {
        hyper_openssl::OpensslClient::new()
            .map(hyper::net::HttpsConnector::new)
            .map(Client::with_connector)
    }
}

#[cfg(feature = "hyper-rustls")]
mod default_client {
    extern crate hyper_rustls;

    pub use util::Never as Error;

    use hyper::Client;
    use hyper::net::HttpsConnector;

    pub fn new() -> Result<Client, Error> {
        Ok(Client::with_connector(HttpsConnector::new(hyper_rustls::TlsClient::new())))
    }
}

#[cfg(not(any(
    feature = "hyper-native-tls",
    feature = "hyper-openssl",
    feature = "hyper-rustls",
)))]
mod default_client {
    pub use util::Never as Error;

    use hyper::Client;

    pub fn new() -> Result<Client, Error> {
        Ok(Client::new())
    }
}
