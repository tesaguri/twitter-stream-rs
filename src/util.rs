use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead, ErrorKind, Read};
use std::mem;
use std::time::Duration;

use bytes::{Buf, IntoBuf, Reader};
use futures::{Async, Future, Poll, Stream};
use tokio_io::{try_nb, AsyncRead};
use tokio_timer;

use error::{Error, HyperError};

// Synonym of `twitter_stream_message::util::string_enums`
macro_rules! string_enums {
    (
        $(
            $(#[$attr:meta])*
            pub enum $E:ident {
                $(
                    $(#[$v_attr:meta])*
                    $V:ident($by:expr)
                ),*;
                $(#[$u_attr:meta])*
                $U:ident(_),
            }
        )*
    ) => {
        $(
            $(#[$attr])*
            pub enum $E {
                $(
                    $(#[$v_attr])*
                    $V,
                )*
                $(#[$u_attr])*
                $U(String),
            }

            impl std::convert::AsRef<str> for $E {
                fn as_ref(&self) -> &str {
                    match *self {
                        $($E::$V => $by,)*
                        $E::$U(ref s) => s,
                    }
                }
            }

            impl std::fmt::Display for $E {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    AsRef::<str>::as_ref(self).fmt(f)
                }
            }

            impl std::cmp::PartialEq for $E {
                fn eq(&self, other: &$E) -> bool {
                    match *self {
                        $($E::$V => match *other {
                            $E::$V => true,
                            $E::$U(ref s) if $by == s => true,
                            _ => false,
                        },)*
                        $E::$U(ref s) => match *other {
                            $($E::$V => $by == s,)*
                            $E::$U(ref t) => s == t,
                        },
                    }
                }
            }

            impl std::hash::Hash for $E {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    match *self {
                        $($E::$V => $by.hash(state),)*
                        $E::$U(ref s) => s.hash(state),
                    }
                }
            }

            impl std::cmp::Eq for $E {}
        )*
    }
}

pub enum Either<A, B> {
    A(A),
    B(B),
}

pub struct Lines<B> {
    read: B,
    line: Vec<u8>,
}

/// Wrapper to map `T::Error` onto `Error`.
pub struct MapErr<T>(T);

pub struct StreamRead<S: Stream>
where
    S::Item: IntoBuf,
{
    body: S,
    buf: Reader<<S::Item as IntoBuf>::Buf>,
    error: Option<S::Error>,
}

/// A `tokio_timer::Timeout`-like type that holds the original `Duration` which can be used to
/// create another `Timeout`.
pub struct Timeout<F> {
    tokio: tokio_timer::Timeout<F>,
    dur: Duration,
}

pub type MaybeTimeout<F> = futures::future::Either<Timeout<F>, MapErr<F>>;

pub type MaybeTimeoutStream<S> = Either<MapErr<tokio_timer::Timeout<S>>, MapErr<S>>;

pub trait IntoError {
    fn into_error(self) -> Error;
}

impl<A: Read, B: Read> Read for Either<A, B> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            Either::A(ref mut a) => a.read(buf),
            Either::B(ref mut b) => b.read(buf),
        }
    }
}

impl<A: BufRead, B: BufRead> BufRead for Either<A, B> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        match *self {
            Either::A(ref mut a) => a.fill_buf(),
            Either::B(ref mut b) => b.fill_buf(),
        }
    }

    fn consume(&mut self, amt: usize) {
        match *self {
            Either::A(ref mut a) => a.consume(amt),
            Either::B(ref mut b) => b.consume(amt),
        }
    }
}

impl<A: AsyncRead, B: AsyncRead> AsyncRead for Either<A, B> {}

impl<A, B> Stream for Either<A, B>
where
    A: Stream,
    B: Stream<Item = A::Item, Error = A::Error>,
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match *self {
            Either::A(ref mut a) => a.poll(),
            Either::B(ref mut b) => b.poll(),
        }
    }
}

impl<B> Lines<B> {
    pub fn new(read: B) -> Self {
        Lines {
            read,
            line: Vec::new(),
        }
    }

    pub fn get_mut(&mut self) -> &mut B {
        &mut self.read
    }
}

impl<B: BufRead> Stream for Lines<B> {
    type Item = Vec<u8>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            let n = try_nb!(self.read.read_until(b'\n', &mut self.line));
            if self.line.ends_with(b"\r\n") {
                // TODO: remove CRLF in v0.10
                // let len = self.line.len();
                // self.line.truncate(len - 2);
                break;
            }
            if n == 0 {
                if self.line.is_empty() {
                    return Ok(None.into());
                } else {
                    break;
                }
            }
            // Not EOF nor CRLF, continue reading.
        }

        Ok(Some(mem::replace(&mut self.line, Vec::new())).into())
    }
}

impl<F: Future> Future for MapErr<F>
where
    F::Error: IntoError,
{
    type Item = F::Item;
    type Error = Error;

    fn poll(&mut self) -> Poll<F::Item, Error> {
        self.0.poll().map_err(IntoError::into_error)
    }
}

impl<S: Stream> Stream for MapErr<S>
where
    S::Error: IntoError,
{
    type Item = S::Item;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, Error> {
        self.0.poll().map_err(IntoError::into_error)
    }
}

impl<S: Stream> StreamRead<S>
where
    S::Item: IntoBuf + Default,
{
    pub fn new(body: S) -> Self {
        StreamRead {
            body,
            buf: S::Item::default().into_buf().reader(),
            error: None,
        }
    }

    pub fn take_err(&mut self) -> Option<S::Error> {
        self.error.take()
    }
}

impl<S: Stream> Read for StreamRead<S>
where
    S::Item: IntoBuf,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.fill_buf()?;
        self.buf.read(buf)
    }
}

impl<S: Stream> BufRead for StreamRead<S>
where
    S::Item: IntoBuf,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        while !self.buf.get_ref().has_remaining() {
            match self.body.poll() {
                Ok(Async::Ready(Some(data))) => self.buf = data.into_buf().reader(),
                Ok(Async::Ready(None)) => break,
                Ok(Async::NotReady) => return Err(ErrorKind::WouldBlock.into()),
                Err(e) => {
                    self.error = Some(e);
                    return Err(ErrorKind::Other.into());
                }
            }
        }

        Ok(self.buf.get_ref().bytes())
    }

    fn consume(&mut self, amt: usize) {
        BufRead::consume(&mut self.buf, amt)
    }
}

impl<S: Stream> AsyncRead for StreamRead<S> where S::Item: IntoBuf {}

impl<F> Timeout<F> {
    pub fn new(fut: F, dur: Duration) -> Self {
        Timeout {
            tokio: tokio_timer::Timeout::new(fut, dur),
            dur,
        }
    }
}

impl<F: Future> Future for Timeout<F>
where
    F::Error: IntoError,
{
    type Item = F::Item;
    type Error = Error;

    fn poll(&mut self) -> Poll<F::Item, Error> {
        MapErr(&mut self.tokio).poll()
    }
}

impl IntoError for Error {
    fn into_error(self) -> Error {
        self
    }
}

impl IntoError for HyperError {
    fn into_error(self) -> Error {
        Error::Hyper(self)
    }
}

impl<E: IntoError> IntoError for tokio_timer::timeout::Error<E> {
    fn into_error(self) -> Error {
        self.into_inner().map_or(Error::TimedOut, E::into_error)
    }
}

pub fn fmt_join<T: Display>(t: &[T], sep: &str, f: &mut Formatter<'_>) -> fmt::Result {
    let mut iter = t.iter();
    if let Some(t) = iter.next() {
        Display::fmt(t, f)?;
        for t in iter {
            write!(f, "{}{}", sep, t)?;
        }
    }
    Ok(())
}

const COMMA: &str = "%2C";

pub fn fmt_follow(ids: &[u64], f: &mut Formatter<'_>) -> fmt::Result {
    fmt_join(ids, COMMA, f)
}

type Location = ((f64, f64), (f64, f64));

pub fn fmt_locations(locs: &[Location], f: &mut Formatter<'_>) -> fmt::Result {
    use std::mem::size_of;
    use std::slice;

    use static_assertions::const_assert;

    let locs: &[f64] = unsafe {
        let ptr: *const Location = locs.as_ptr();
        const_assert!(size_of::<Location>() % size_of::<f64>() == 0);
        let n = locs.len() * (size_of::<Location>() / size_of::<f64>());
        slice::from_raw_parts(ptr as *const f64, n)
    };

    fmt_join(locs, COMMA, f)
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub fn not(p: &bool) -> bool {
    !p
}

pub fn timeout<F>(fut: F, dur: Option<Duration>) -> MaybeTimeout<F> {
    use futures::future::Either;

    match dur {
        Some(dur) => Either::A(Timeout::new(fut, dur)),
        None => Either::B(MapErr(fut)),
    }
}

pub fn timeout_to_stream<F, S>(timeout: &MaybeTimeout<F>, s: S) -> MaybeTimeoutStream<S> {
    use futures::future::Either as EitherFut;

    match *timeout {
        EitherFut::A(ref t) => Either::A(MapErr(tokio_timer::Timeout::new(s, t.dur))),
        EitherFut::B(_) => Either::B(MapErr(s)),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use bytes::Bytes;
    use futures::stream;

    use super::*;

    const BUF_SIZE: usize = 8 * 1024;

    #[test]
    fn lines() {
        let lines = [
            "abc\r\n",
            "d\r\n",
            "efg\r\n",
            "hijk\r\n",
            "\r\n",
            "lmn\r\n",
            "opq\rrs\r\n",
            "\n\rtuv\r\r\n",
            "wxyz\n",
        ];
        let body = [
            b"abc\r\n" as &[_],
            b"d\r\nefg\r\n" as &[_],
            b"hi" as &[_],
            b"jk" as &[_],
            b"" as &[_],
            b"\r\n" as &[_],
            b"\r\n" as &[_],
            b"lmn\r\nop" as &[_],
            b"q\rrs\r" as &[_],
            b"\n\n\rtuv\r\r\n" as &[_],
            b"wxyz\n" as &[_],
        ];

        let mut iter1 = lines.iter().cloned();
        let mut iter2 = Lines::new(StreamRead::new(stream::iter_ok::<_, ()>(
            body.iter().cloned(),
        )))
        .wait()
        .map(|s| String::from_utf8(s.unwrap().to_vec()).unwrap());

        for _ in 0..(lines.len() + 1) {
            assert_eq!(iter1.next(), iter2.next().as_ref().map(String::as_str));
        }
    }

    #[test]
    fn stream_read() {
        macro_rules! assert_id {
            ($($elm:expr),*) => {{
                let elms = [$(Bytes::from(&$elm[..])),*];
                let mut buf = Vec::new();
                StreamRead {
                    body: stream::iter_ok::<_,()>(
                        elms.iter().cloned().map(Cursor::new)
                    ),
                    buf: Cursor::new(Bytes::new()).reader(),
                    error: None,
                }.read_to_end(&mut buf).unwrap();
                assert_eq!(buf, elms.concat());
            }};
        }
        assert_id!();
        assert_id!([]);
        assert_id!([], [0], [], [], [1, 2]);
        assert_id!([0; BUF_SIZE + 1], [1; BUF_SIZE - 1], [2; BUF_SIZE * 5 / 2]);
    }
}
