use std::io;
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_compression::stream::GzipDecoder;
use bytes::Bytes;
use futures_core::{Stream, TryStream};
use futures_util::future::Either;
use futures_util::{ready, StreamExt, TryStreamExt};

use crate::error::Error;

pub struct Gzip<S: TryStream<Ok = Bytes> + Unpin>(GzipDecoder<Adapter<S>>)
where
    S::Error: Unpin;

struct Adapter<S>
where
    S: TryStream,
{
    inner: S,
    error: Option<S::Error>,
}

pub type MaybeGzip<S> = Either<Gzip<S>, S>;

impl<S: TryStream<Ok = Bytes> + Unpin> Gzip<S>
where
    S::Error: Unpin,
{
    fn new(s: S) -> Self {
        Gzip(GzipDecoder::new(Adapter {
            inner: s,
            error: None,
        }))
    }
}

impl<S: TryStream<Ok = Bytes, Error = Error> + Unpin> Stream for Gzip<S> {
    type Item = Result<Bytes, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // This cannot be `(&mut self.0).map_ok(..).map_err(..).poll_next_unpin(..)`
        // because `self` is borrowed inside `map_err`.
        (&mut self.0).poll_next_unpin(cx).map(|option| {
            option.map(|result| {
                result.map_err(|e| self.0.get_mut().error.take().unwrap_or(Error::Gzip(e)))
            })
        })
    }
}

impl<S: TryStream<Ok = Bytes> + Unpin> Stream for Adapter<S>
where
    S::Error: Unpin,
{
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match ready!({
            let inner = &mut self.inner;
            inner.try_poll_next_unpin(cx)
        }) {
            Some(result) => Poll::Ready(Some(result.map(Into::into).map_err(|e| {
                self.error = Some(e);
                io::Error::from_raw_os_error(0)
            }))),
            None => Poll::Ready(None),
        }
    }
}

pub fn gzip<S: TryStream<Ok = Bytes> + Unpin>(s: S) -> MaybeGzip<S>
where
    S::Error: Unpin,
{
    Either::Left(Gzip::new(s))
}

pub fn identity<S: TryStream<Ok = Bytes> + Unpin>(s: S) -> MaybeGzip<S>
where
    S::Error: Unpin,
{
    Either::Right(s)
}
