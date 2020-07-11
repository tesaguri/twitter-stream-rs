#![doc(html_root_url = "https://docs.rs/twitter-stream/0.10.0")]

/*!
# Twitter Stream

A library for listening on Twitter Streaming API.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
futures = "0.3"
tokio = { version = "0.2", features = ["macros"] }
twitter-stream = "0.10"
```

## Overview

Here is a basic example that prints public mentions to @Twitter in JSON format:

```no_run
use futures::prelude::*;
use twitter_stream::{Token, TwitterStream};

# #[tokio::main]
# async fn main() {
let token = Token::new("consumer_key", "consumer_secret", "access_key", "access_secret");

TwitterStream::track("@Twitter", &token)
    .try_flatten_stream()
    .try_for_each(|json| {
        println!("{}", json);
        future::ok(())
    })
    .await
    .unwrap();
# }
```

See the [`TwitterStream`] type documentation for details.

## Streaming messages

`TwitterStream` yields the raw JSON strings returned by the Streaming API. Each string value
contains exactly one JSON value.

The underlying Streaming API [sends a blank line][stalls] every 30 seconds as a "keep-alive" signal,
but `TwitterStream` discards it so that you can always expect to yield a valid JSON string.
On the other hand, this means that you cannot use the blank line to set a timeout on `Stream`-level.
If you want the stream to time out on network stalls, set a timeout on the underlying
HTTP connector, instead of the `Stream` (see the [`timeout` example] in the crate's repository
for details).

[stalls]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/connecting#stalls
[`timeout` example]: https://github.com/tesaguri/twitter-stream-rs/blob/v0.10.0/examples/timeout.rs

The JSON string usually, but not always, represents a [Tweet] object. When deserializing the JSON
string, you should be able to handle any kind of JSON value. A possible implementation of
deserialization would be like the following:

[Tweet]: https://developer.twitter.com/en/docs/tweets/data-dictionary/overview/tweet-object

```
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum StreamMessage {
    Tweet(Tweet),
    // Discards anything other than a Tweet.
    // You can handle other message types as well by adding correspoiding variants.
    Other(serde::de::IgnoredAny),
}

#[derive(serde::Deserialize)]
struct Tweet { /* ... */ }
```

The [`echo_bot` example] in the crate's repository shows an example of a `StreamMessage`
implementation.

[`echo_bot` example]: https://github.com/tesaguri/twitter-stream-rs/blob/v0.10.0/examples/echo_bot.rs

See the [Twitter Developers Documentation][message-types] for the types and formats of the JSON
messages.

[message-types]: https://developer.twitter.com/en/docs/tweets/filter-realtime/guides/streaming-message-types
*/

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(intra_doc_link_resolution_failure)]
#![warn(missing_docs)]

#[macro_use]
mod util;

pub mod builder;
pub mod error;
#[cfg(feature = "hyper")]
#[cfg_attr(docsrs, doc(cfg(feature = "hyper")))]
pub mod hyper;
pub mod service;

mod gzip;
mod token;

pub use oauth::Credentials;

pub use crate::builder::Builder;
pub use crate::error::Error;
pub use crate::token::Token;

use std::future::Future;
use std::pin::Pin;
use std::str;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_core::Stream;
use futures_util::ready;
use http::header::CONTENT_ENCODING;
use http::response::Parts;
use http::Response;
use http::StatusCode;
use http_body::Body;
use pin_project_lite::pin_project;

use crate::gzip::MaybeGzip;
use crate::util::{HttpBodyAsStream, Lines};

pin_project! {
    /// A future returned by constructor methods which resolves to a [`TwitterStream`].
    pub struct FutureTwitterStream<F> {
        #[pin]
        response: F,
    }
}

pin_project! {
    /// A listener for Twitter Streaming API, yielding JSON strings returned from the API.
    pub struct TwitterStream<B: Body> {
        #[pin]
        inner: Lines<MaybeGzip<HttpBodyAsStream<B>>>,
    }
}

impl<B: Body> TwitterStream<B> {
    /// Creates a `Builder` for `TwitterStream`.
    pub fn builder<'a, C, A>(token: Token<C, A>) -> Builder<'a, Token<C, A>>
    where
        C: std::borrow::Borrow<str>,
        A: std::borrow::Borrow<str>,
    {
        Builder::new(token)
    }
}

#[cfg(feature = "hyper")]
impl crate::hyper::TwitterStream {
    /// Connect to the filter stream, yielding Tweets from the users specified by `follow` argument.
    ///
    /// This is a shorthand for `twitter_stream::Builder::new(token).follow(follow).listen()`.
    /// For more specific configurations, use [`TwitterStream::builder`] or [`Builder::new`].
    ///
    /// # Panics
    ///
    /// This will panic if the underlying HTTPS connector failed to initialize.
    pub fn follow<C, A>(follow: &[u64], token: &Token<C, A>) -> crate::hyper::FutureTwitterStream
    where
        C: std::borrow::Borrow<str>,
        A: std::borrow::Borrow<str>,
    {
        Builder::new(token.as_ref()).follow(follow).listen()
    }

    /// Connect to the filter stream, yielding Tweets that matches the query specified by
    /// `track` argument.
    ///
    /// This is a shorthand for `twitter_stream::Builder::new(token).track(track).listen()`.
    /// For more specific configurations, use [`TwitterStream::builder`] or [`Builder::new`].
    ///
    /// # Panics
    ///
    /// This will panic if the underlying HTTPS connector failed to initialize.
    pub fn track<C, A>(track: &str, token: &Token<C, A>) -> crate::hyper::FutureTwitterStream
    where
        C: std::borrow::Borrow<str>,
        A: std::borrow::Borrow<str>,
    {
        Builder::new(token.as_ref()).track(track).listen()
    }

    /// Connect to the filter stream, yielding geolocated Tweets falling within the specified
    /// bounding boxes.
    ///
    /// This is a shorthand for `twitter_stream::Builder::new(token).locations(locations).listen()`.
    /// For more specific configurations, use [`TwitterStream::builder`] or [`Builder::new`].
    ///
    /// # Panics
    ///
    /// This will panic if the underlying HTTPS connector failed to initialize.
    pub fn locations<C, A>(
        locations: &[builder::BoundingBox],
        token: &Token<C, A>,
    ) -> crate::hyper::FutureTwitterStream
    where
        C: std::borrow::Borrow<str>,
        A: std::borrow::Borrow<str>,
    {
        Builder::new(token.as_ref()).locations(locations).listen()
    }

    /// Connect to the sample stream, yielding a "small random sample" of all public Tweets.
    ///
    /// This is a shorthand for `twitter_stream::Builder::new(token).listen()`.
    /// For more specific configurations, use [`TwitterStream::builder`] or [`Builder::new`].
    ///
    /// # Panics
    ///
    /// This will panic if the underlying HTTPS connector failed to initialize.
    pub fn sample<C, A>(token: &Token<C, A>) -> crate::hyper::FutureTwitterStream
    where
        C: std::borrow::Borrow<str>,
        A: std::borrow::Borrow<str>,
    {
        Builder::new(token.as_ref()).listen()
    }
}

impl<F, B, E> Future for FutureTwitterStream<F>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: Body,
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
            Lines::new(gzip::gzip(HttpBodyAsStream::new(body)))
        } else {
            Lines::new(gzip::identity(HttpBodyAsStream::new(body)))
        };

        Poll::Ready(Ok(TwitterStream { inner }))
    }
}

impl<B> Stream for TwitterStream<B>
where
    B: Body,
{
    type Item = Result<string::String<Bytes>, Error<B::Error>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            let line = match ready!(this.inner.as_mut().poll_next(cx)?) {
                Some(t) => t,
                None => return std::task::Poll::Ready(None),
            };

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
