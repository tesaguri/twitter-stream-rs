// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org/>

// This line shouldn't be necessary in a real project.
extern crate hyper_pkg as hyper;

use std::time::Duration;

use futures::prelude::*;
use twitter_stream::Token;

const TIMEOUT: Duration = Duration::from_secs(90);

#[tokio::main]
async fn main() {
    let token = Token::new(
        "consumer_key",
        "consumer_secret",
        "access_key",
        "access_secret",
    );

    let mut conn = hyper::client::HttpConnector::new();
    conn.set_connect_timeout(Some(TIMEOUT));
    let tls = native_tls::TlsConnector::new().unwrap();
    let conn = hyper_tls::HttpsConnector::from((conn, tls.into()));

    let client = hyper::Client::builder().build::<_, hyper::Body>(conn);
    let client = timeout::Timeout::new(client, TIMEOUT);

    let result = twitter_stream::Builder::filter(token)
        .track(Some("@Twitter"))
        .listen_with_client(client)
        .try_flatten_stream()
        .try_for_each(|json| {
            println!("{}", json);
            future::ok(())
        })
        .await;

    match result {
        Ok(()) => {}
        Err(twitter_stream::Error::Service(timeout::Error::Elapsed)) => eprintln!("timed out"),
        Err(e) => eprintln!("error: {:?}", e),
    }
}

mod timeout {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::time::Duration;

    use futures::TryFuture;

    /// A wrapper for an HTTP client service that applies a read timeout to its response body.
    pub struct Timeout<S> {
        inner: S,
        timeout: Duration,
    }

    #[pin_project::pin_project]
    pub struct Body<T> {
        #[pin]
        inner: T,
        timeout: Duration,
        #[pin]
        delay: tokio::time::Delay,
    }

    #[derive(Debug)]
    pub enum Error<E> {
        Inner(E),
        Elapsed,
    }

    #[pin_project::pin_project]
    pub struct ResponseFuture<F> {
        #[pin]
        inner: F,
        timeout: Duration,
    }

    impl<S> Timeout<S> {
        pub fn new(service: S, timeout: Duration) -> Self {
            Timeout {
                inner: service,
                timeout,
            }
        }
    }

    impl<S, T, B> tower_service::Service<T> for Timeout<S>
    where
        S: tower_service::Service<T, Response = http::Response<B>>,
    {
        type Response = http::Response<Body<B>>;
        type Error = Error<S::Error>;
        type Future = ResponseFuture<S::Future>;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.inner.poll_ready(cx).map_err(Error::Inner)
        }

        fn call(&mut self, req: T) -> Self::Future {
            ResponseFuture {
                inner: self.inner.call(req),
                timeout: self.timeout,
            }
        }
    }

    impl<B> http_body::Body for Body<B>
    where
        B: http_body::Body,
    {
        type Data = B::Data;
        type Error = Error<B::Error>;

        fn poll_data(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
            let mut this = self.project();

            if let Poll::Ready(o) = this.inner.poll_data(cx) {
                let deadline = tokio::time::Instant::now() + *this.timeout;
                this.delay.reset(deadline);
                return Poll::Ready(o.map(|r| r.map_err(Error::Inner)));
            }

            this.delay.poll(cx).map(|()| Some(Err(Error::Elapsed)))
        }

        fn poll_trailers(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
            let this = self.project();

            if let Poll::Ready(r) = this.inner.poll_trailers(cx) {
                return Poll::Ready(r.map_err(Error::Inner));
            }

            this.delay.poll(cx).map(|()| Err(Error::Elapsed))
        }
    }

    impl<F, B> Future for ResponseFuture<F>
    where
        F: TryFuture<Ok = http::Response<B>>,
    {
        type Output = Result<http::Response<Body<B>>, Error<F::Error>>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.as_mut().project();
            this.inner.try_poll(cx).map_err(Error::Inner).map_ok(|res| {
                let (parts, body) = res.into_parts();
                let body = Body {
                    inner: body,
                    timeout: self.timeout,
                    delay: tokio::time::delay_for(self.timeout),
                };
                http::Response::from_parts(parts, body)
            })
        }
    }
}
