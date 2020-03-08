pub use imp::{gzip, identity, MaybeGzip};

#[cfg(feature = "gzip")]
mod imp {
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use async_compression::stream::GzipDecoder;
    use bytes::Bytes;
    use futures_core::{Stream, TryStream};
    use futures_util::future::Either;
    use futures_util::ready;
    use pin_project_lite::pin_project;

    use crate::error::Error;

    pub type MaybeGzip<S> = Either<Gzip<S>, S>;

    pin_project! {
        pub struct Gzip<S: TryStream<Ok = Bytes>> {
            #[pin]
            inner: GzipDecoder<Adapter<S>>,
        }
    }

    pin_project! {
        struct Adapter<S>
        where
            S: TryStream,
        {
            #[pin]
            inner: S,
            error: Option<S::Error>,
        }
    }

    impl<S: TryStream<Ok = Bytes>> Gzip<S> {
        fn new(s: S) -> Self {
            Gzip {
                inner: GzipDecoder::new(Adapter {
                    inner: s,
                    error: None,
                }),
            }
        }
    }

    impl<S: TryStream<Ok = Bytes, Error = Error<E>>, E> Stream for Gzip<S> {
        type Item = Result<Bytes, Error<E>>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let mut this = self.project();
            // This cannot be `this.map_ok(..).map_err(..).poll_next(..)`
            // because `this` is borrowed inside `map_err`.
            this.inner.as_mut().poll_next(cx).map(|option| {
                option.map(|result| {
                    result.map_err(|e| {
                        this.inner
                            .get_pin_mut()
                            .project()
                            .error
                            .take()
                            .unwrap_or(Error::Gzip(e))
                    })
                })
            })
        }
    }

    impl<S: TryStream<Ok = Bytes>> Stream for Adapter<S> {
        type Item = io::Result<Bytes>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let mut this = self.project();
            match ready!(this.inner.as_mut().try_poll_next(cx)) {
                Some(result) => Poll::Ready(Some(result.map(Into::into).map_err(|e| {
                    *this.error = Some(e);
                    io::Error::from_raw_os_error(0)
                }))),
                None => Poll::Ready(None),
            }
        }
    }

    pub fn gzip<S: TryStream<Ok = Bytes>>(s: S) -> MaybeGzip<S> {
        Either::Left(Gzip::new(s))
    }

    pub fn identity<S: TryStream<Ok = Bytes>>(s: S) -> MaybeGzip<S> {
        Either::Right(s)
    }
}

#[cfg(not(feature = "gzip"))]
mod imp {
    use bytes::Bytes;
    use futures_core::TryStream;

    pub type MaybeGzip<S> = S;

    pub fn gzip<S: TryStream<Ok = Bytes>>(s: S) -> MaybeGzip<S> {
        s
    }

    pub fn identity<S: TryStream<Ok = Bytes>>(s: S) -> MaybeGzip<S> {
        s
    }
}
