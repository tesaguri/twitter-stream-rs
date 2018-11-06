use std::error;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use bytes::Bytes;
use futures::future::Either;
use futures::stream::Fuse;
use futures::{try_ready, Future, Poll, Stream};
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

pub enum EitherStream<A, B> {
    A(A),
    B(B),
}

pub struct JoinDisplay<'a, D: 'a, Sep: ?Sized + 'a>(pub &'a [D], pub &'a Sep);

/// A stream over the lines (delimited by CRLF) of a `Body`.
pub struct Lines<B>
where
    B: Stream,
{
    inner: Fuse<B>,
    buf: Bytes,
}

/// Wrapper to map `T::Error` onto `Error`.
pub struct MapErr<T>(T);

/// A `tokio_timer::Timeout`-like type that holds the original `Duration` which can be used to
/// create another `Timeout`.
pub struct Timeout<F> {
    tokio: tokio_timer::Timeout<F>,
    dur: Duration,
}

pub type MaybeTimeout<F> = Either<Timeout<F>, MapErr<F>>;

pub type MaybeTimeoutStream<S> = EitherStream<MapErr<tokio_timer::Timeout<S>>, MapErr<S>>;

/// Represents an infallible operation in `default_connector::new`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Never {}

pub trait IntoError {
    fn into_error(self) -> Error;
}

impl<A, B> Stream for EitherStream<A, B>
where
    A: Stream,
    B: Stream<Item = A::Item, Error = A::Error>,
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self {
            EitherStream::A(ref mut a) => a.poll(),
            EitherStream::B(ref mut b) => b.poll(),
        }
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

impl<'a, D, Sep> Display for JoinDisplay<'a, D, Sep>
where
    D: Display + 'a,
    Sep: Display + ?Sized + 'a,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut iter = self.0.iter();
        if let Some(d) = iter.next() {
            Display::fmt(d, f)?;
            for d in iter {
                write!(f, "{}{}", self.1, d)?;
            }
        }
        Ok(())
    }
}

impl<B> Lines<B>
where
    B: Stream,
{
    pub fn new(body: B) -> Self {
        Lines {
            inner: body.fuse(),
            buf: Bytes::new(),
        }
    }
}

impl<B> Stream for Lines<B>
where
    B: Stream,
    B::Item: Into<Bytes>,
{
    type Item = Bytes;
    type Error = B::Error;

    fn poll(&mut self) -> Poll<Option<Bytes>, B::Error> {
        use std::mem;

        macro_rules! try_ready_some {
            ($poll:expr) => {{
                let poll = $poll;
                match try_ready!(poll) {
                    Some(v) => v,
                    None => return Ok(None.into()),
                }
            }};
        }

        if self.buf.is_empty() {
            self.buf = try_ready_some!(self.inner.poll()).into();
        }

        fn remove_first_line(buf: &mut Bytes) -> Option<Bytes> {
            (buf as &Bytes)
                .into_iter()
                .enumerate()
                .find(|&(i, b)| b'\r' == b && Some(&b'\n') == buf.get(i + 1))
                .map(|(i, _)| buf.split_to(i + 2))
        }

        if let Some(buf) = remove_first_line(&mut self.buf) {
            return Ok(Some(buf).into());
        }

        if self.buf.is_empty() {
            self.buf = try_ready_some!(self.inner.poll()).into();
        }

        // Extend the buffer until a newline is found:
        loop {
            match try_ready!(self.inner.poll()) {
                Some(chunk) => {
                    let mut chunk = chunk.into();
                    if b'\r' == *self.buf.last().unwrap() && Some(&b'\n') == chunk.first() {
                        let i = self.buf.len() + 1;
                        self.buf.extend_from_slice(&chunk);
                        let next = self.buf.split_off(i);
                        let ret = mem::replace(&mut self.buf, next);
                        return Ok(Some(ret).into());
                    } else if let Some(tail) = remove_first_line(&mut chunk) {
                        self.buf.extend_from_slice(&tail);
                        let ret = mem::replace(&mut self.buf, chunk);
                        return Ok(Some(ret).into());
                    } else {
                        self.buf.extend_from_slice(&chunk);
                    }
                }
                None => {
                    let ret = mem::replace(&mut self.buf, Bytes::new());
                    return Ok(Some(ret).into());
                }
            };
        }
    }
}

impl error::Error for Never {
    fn description(&self) -> &str {
        match *self {}
    }
}

impl Display for Never {
    fn fmt(&self, _: &mut Formatter<'_>) -> fmt::Result {
        match *self {}
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
        match self.into_inner() {
            Some(e) => e.into_error(),
            None => Error::TimedOut,
        }
    }
}

pub fn timeout<F>(fut: F, dur: Option<Duration>) -> MaybeTimeout<F> {
    match dur {
        Some(dur) => Either::A(Timeout::new(fut, dur)),
        None => Either::B(MapErr(fut)),
    }
}

pub fn timeout_to_stream<F, S>(timeout: &MaybeTimeout<F>, s: S) -> MaybeTimeoutStream<S> {
    match *timeout {
        Either::A(ref t) => EitherStream::A(MapErr(tokio_timer::Timeout::new(s, t.dur))),
        Either::B(_) => EitherStream::B(MapErr(s)),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;
    use futures::stream;

    #[test]
    fn lines() {
        let lines = vec![
            "abc\r\n",
            "d\r\n",
            "efg\r\n",
            "hijk\r\n",
            "\r\n",
            "lmn\r\n",
            "opq\rrs\r\n",
            "\n\rtuv\r\r\n",
            "wxyz",
        ];
        let body = vec![
            Bytes::from(b"abc\r\n".to_vec()),
            Bytes::from(b"d\r\nefg\r\n".to_vec()),
            Bytes::from(b"hi".to_vec()),
            Bytes::from(b"jk".to_vec()),
            Bytes::from(b"".to_vec()),
            Bytes::from(b"\r\n".to_vec()),
            Bytes::from(b"\r\n".to_vec()),
            Bytes::from(b"lmn\r\nop".to_vec()),
            Bytes::from(b"q\rrs\r".to_vec()),
            Bytes::from(b"\n\n\rtuv\r\r\n".to_vec()),
            Bytes::from(b"wxyz".to_vec()),
        ];

        let mut iter1 = lines.iter().cloned();
        let mut iter2 = Lines::new(stream::iter_ok::<_, ()>(body))
            .wait()
            .map(|s| String::from_utf8(s.unwrap().to_vec()).unwrap());

        for _ in 0..(lines.len() + 1) {
            assert_eq!(iter1.next(), iter2.next().as_ref().map(String::as_str));
        }
    }
}
