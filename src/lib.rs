#![doc(html_root_url = "https://docs.rs/twitter-stream/0.10.0-alpha.5")]

/*!
# Twitter Stream

A library for listening on Twitter Streaming API.

## Usage

Add `twitter-stream` to your dependencies in your project's `Cargo.toml`:

```toml
[dependencies]
futures = "0.3"
tokio = { version = "0.2", features = ["macros"] }
twitter-stream = "=0.10.0-alpha.5"
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

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(intra_doc_link_resolution_failure)]

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

use std::borrow::Borrow;
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
    /// A future returned by constructor methods
    /// which resolves to a `TwitterStream`.
    pub struct FutureTwitterStream<F> {
        #[pin]
        response: F,
    }
}

pin_project! {
    /// A listener for Twitter Streaming API.
    /// It yields JSON strings returned from the API.
    pub struct TwitterStream<B: Body> {
        #[pin]
        inner: Lines<MaybeGzip<HttpBodyAsStream<B>>>,
    }
}

#[cfg(feature = "hyper")]
impl<B: Body> TwitterStream<B> {
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
