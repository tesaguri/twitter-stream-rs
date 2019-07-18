use std::io::{self, BufReader, Read};

use bytes::buf::IntoBuf;
use futures::Stream;
use libflate::non_blocking::gzip;
use tokio_io::AsyncRead;

use crate::util::{Either, StreamRead};

pub struct Gzip<S>
where
    S: Stream,
    S::Item: IntoBuf,
{
    inner: gzip::Decoder<StreamRead<S>>,
}

pub type MaybeGzip<S> = Either<BufReader<Gzip<S>>, StreamRead<S>>;

impl<S: Stream> Gzip<S>
where
    S::Item: IntoBuf + Default,
{
    pub fn new(body: S) -> Self {
        Gzip {
            inner: gzip::Decoder::new(StreamRead::new(body)),
        }
    }

    pub fn take_err(&mut self) -> Option<S::Error> {
        self.inner.as_inner_mut().take_err()
    }
}

impl<S: Stream> AsyncRead for Gzip<S> where S::Item: IntoBuf {}

impl<S: Stream> Read for Gzip<S>
where
    S::Item: IntoBuf,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<S: Stream> MaybeGzip<S>
where
    S::Item: IntoBuf + Default,
{
    pub fn gzip(s: S) -> Self {
        Either::A(BufReader::new(Gzip::new(s)))
    }

    pub fn identity(s: S) -> Self {
        Either::B(StreamRead::new(s))
    }
}
