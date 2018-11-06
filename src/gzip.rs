use std::io::{self, ErrorKind, Read};

use bytes::buf::{Buf, BufMut, Reader};
use bytes::BytesMut;
use futures::{Async, Poll, Stream};
use hyper::body::Chunk;
use libflate::non_blocking::gzip;

use util::EitherStream;
use Error;

pub struct Gzip<S>
where
    S: Stream,
{
    inner: ReadStream<gzip::Decoder<StreamRead<S>>>,
}

pub type MaybeGzip<S> = EitherStream<Gzip<S>, S>;

struct StreamRead<S>
where
    S: Stream,
{
    body: S,
    buf: Reader<S::Item>,
    error: Option<S::Error>,
}

struct ReadStream<R> {
    inner: R,
    buf: BytesMut,
}

impl<S: Stream> Gzip<S>
where
    S::Item: Buf + Default,
{
    pub fn new(body: S) -> Self {
        Gzip {
            inner: ReadStream::new(gzip::Decoder::new(StreamRead::new(body))),
        }
    }
}

impl<S: Stream<Error = Error>> Stream for Gzip<S>
where
    S::Item: Buf,
{
    type Item = Chunk;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Chunk>, Error> {
        match self.inner.poll() {
            Ok(async) => Ok(async),
            Err(e) => if let Some(e) = self.inner.inner.as_inner_mut().error.take() {
                Err(e)
            } else {
                Err(Error::Gzip(e))
            },
        }
    }
}

impl<S: Stream> StreamRead<S>
where
    S::Item: Buf + Default,
{
    pub fn new(body: S) -> Self {
        StreamRead {
            body,
            buf: S::Item::default().reader(),
            error: None,
        }
    }
}

impl<S: Stream> Read for StreamRead<S>
where
    S::Item: Buf,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while !self.buf.get_ref().has_remaining() {
            match self.body.poll() {
                Ok(Async::Ready(Some(data))) => self.buf = data.reader(),
                Ok(Async::Ready(None)) => return Ok(0),
                Ok(Async::NotReady) => return Err(ErrorKind::WouldBlock.into()),
                Err(e) => {
                    self.error = Some(e);
                    return Err(ErrorKind::Other.into());
                }
            }
        }
        self.buf.read(buf)
    }
}

const BUF_SIZE: usize = 8 * 1024;

impl<R: Read> ReadStream<R> {
    fn new(read: R) -> Self {
        ReadStream {
            inner: read,
            buf: BytesMut::with_capacity(BUF_SIZE),
        }
    }
}

impl<R: Read> Stream for ReadStream<R> {
    type Item = Chunk;
    type Error = io::Error;

    // This is almost a copy-and-paste from `reqwest` crate:
    // https://github.com/seanmonstar/reqwest/blob/fe8c7a2/src/async_impl/decoder.rs#L179-L214
    fn poll(&mut self) -> Poll<Option<Chunk>, io::Error> {
        if !self.buf.has_remaining_mut() {
            self.buf.reserve(BUF_SIZE);
        }

        let read = unsafe {
            let buf = self.buf.bytes_mut();
            self.inner.read(buf)
        };

        match read {
            Ok(0) => Ok(None.into()),
            Ok(amt) => {
                unsafe {
                    self.buf.advance_mut(amt);
                }
                let chunk = Chunk::from(self.buf.split_to(amt).freeze());
                Ok(Some(chunk).into())
            }
            Err(ref e) if ErrorKind::WouldBlock == e.kind() => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}

impl<S: Stream> MaybeGzip<S>
where
    S::Item: Buf + Default,
{
    pub fn gzip(s: S) -> Self {
        EitherStream::A(Gzip::new(s))
    }

    pub fn identity(s: S) -> Self {
        EitherStream::B(s)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use bytes::Bytes;
    use futures::{stream, Future};

    use super::*;

    #[test]
    fn identity() {
        macro_rules! assert_id {
            ($($elm:expr),*) => {{
                let elms = [$(Bytes::from(&$elm[..])),*];
                assert_eq!(
                    *ReadStream::new(StreamRead {
                        body: stream::iter_ok::<_,()>(
                            elms.iter().cloned().map(Cursor::new)
                        ),
                        buf: Cursor::new(Bytes::new()).reader(),
                        error: None,
                    }).concat2().wait().unwrap(),
                    *elms.concat(),
                );
            }};
        }
        assert_id!();
        assert_id!([]);
        assert_id!([], [0], [], [], [1, 2]);
        assert_id!([0; BUF_SIZE + 1], [1; BUF_SIZE - 1], [2; BUF_SIZE * 5 / 2]);
    }
}
