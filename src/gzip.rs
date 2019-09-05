use std::io;
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_compression::stream::GzipDecoder;
use bytes::Bytes;
use futures_core::{Stream, TryStream};
use futures_util::future::Either;
use futures_util::{ready, StreamExt, TryStreamExt};
use hyper::Chunk;

use crate::error::Error;

pub struct Gzip<S: TryStream + Unpin>(GzipDecoder<Adapter<S, S::Error>>)
where
    S::Ok: Into<Bytes>,
    S::Error: Unpin;

struct Adapter<S, E> {
    inner: S,
    error: Option<E>,
}

pub type MaybeGzip<S> = Either<Gzip<S>, S>;

impl<S: TryStream<Ok = Chunk> + Unpin> Gzip<S>
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

impl<S: TryStream<Error = Error> + Unpin> Stream for Gzip<S>
where
    S::Ok: Into<Bytes>,
    S::Error: Unpin,
{
    type Item = Result<Chunk, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // This cannot be `(&mut self.0).map_ok(..).map_err(..).poll_next_unpin(..)`
        // because `self` is borrowed inside `map_err`.
        (&mut self.0)
            .map_ok(Into::<Bytes>::into)
            .poll_next_unpin(cx)
            .map(|option| {
                option.map(|result| {
                    result
                        .map(Into::<Chunk>::into)
                        .map_err(|e| self.0.get_mut().error.take().unwrap_or(Error::Gzip(e)))
                })
            })
    }
}

impl<S: TryStream + Unpin> Stream for Adapter<S, S::Error>
where
    S::Ok: Into<Bytes>,
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

pub fn gzip<S: TryStream<Ok = Chunk> + Unpin>(s: S) -> MaybeGzip<S>
where
    S::Error: Unpin,
{
    Either::Left(Gzip::new(s))
}

pub fn identity<S: TryStream<Ok = Chunk> + Unpin>(s: S) -> MaybeGzip<S>
where
    S::Error: Unpin,
{
    Either::Right(s)
}
