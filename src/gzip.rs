pub use imp::{gzip, identity, MaybeGzip};

#[cfg(feature = "gzip")]
mod imp {
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use async_compression::tokio::bufread::GzipDecoder;
    use bytes::{Buf, Bytes};
    use futures_core::{Stream, TryStream};
    use futures_util::future::Either;
    use futures_util::ready;
    use pin_project_lite::pin_project;
    use tokio_util::codec::{BytesCodec, FramedRead};
    use tokio_util::io::StreamReader;

    use crate::error::Error;

    pub type MaybeGzip<S> = Either<Gzip<S>, S>;

    type DecoderStream<S> =
        FramedRead<GzipDecoder<StreamReader<S, <S as TryStream>::Ok>>, BytesCodec>;

    pin_project! {
        pub struct Gzip<S: TryStream> {
            #[pin]
            inner: DecoderStream<Adapter<S>>,
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

    impl<S: TryStream> Gzip<S>
    where
        S::Ok: Buf,
    {
        fn new(s: S) -> Self {
            let inner = FramedRead::new(
                GzipDecoder::new(StreamReader::new(Adapter {
                    inner: s,
                    error: None,
                })),
                BytesCodec::new(),
            );
            Gzip { inner }
        }
    }

    impl<S: TryStream<Error = Error<E>>, E> Stream for Gzip<S>
    where
        S::Ok: Buf,
    {
        type Item = Result<Bytes, Error<E>>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let mut inner = self.project().inner;
            match ready!(inner.as_mut().poll_next(cx)) {
                Some(Ok(buf)) => Poll::Ready(Some(Ok(buf.freeze()))),
                Some(Err(e)) => Poll::Ready(Some(Err(inner
                    .get_pin_mut()
                    .get_pin_mut()
                    .get_pin_mut()
                    .project()
                    .error
                    .take()
                    .unwrap_or(Error::Gzip(e))))),
                None => Poll::Ready(None),
            }
        }
    }

    impl<S: TryStream> Stream for Adapter<S> {
        type Item = io::Result<S::Ok>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let mut this = self.project();
            match ready!(this.inner.as_mut().try_poll_next(cx)) {
                Some(result) => Poll::Ready(Some(result.map_err(|e| {
                    *this.error = Some(e);
                    io::Error::from_raw_os_error(0)
                }))),
                None => Poll::Ready(None),
            }
        }
    }

    pub fn gzip<S: TryStream>(s: S) -> MaybeGzip<S>
    where
        S::Ok: Buf,
    {
        Either::Left(Gzip::new(s))
    }

    pub fn identity<S: TryStream>(s: S) -> MaybeGzip<S> {
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
