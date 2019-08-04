use std::fmt::{self, Display, Formatter};
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Bytes, BytesMut};
use cfg_if::cfg_if;
use futures_core::{Stream, TryFuture, TryStream};
use futures_util::future::FutureExt;
use futures_util::ready;
use futures_util::stream::{Fuse, StreamExt};
use futures_util::try_future::TryFutureExt;
use futures_util::try_stream::{IntoStream, TryStreamExt};

use crate::error::{Error, HyperError};

macro_rules! ready_some {
    ($e:expr) => {
        match $e {
            std::task::Poll::Ready(Some(t)) => t,
            std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}

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

pub struct Lines<S> {
    stream: Fuse<IntoStream<S>>,
    buf: BytesMut,
}

/// Wrapper to map `T::Error` onto `Error`.
pub struct MapErr<T>(T);

pub trait IntoError {
    fn into_error(self) -> Error;
}

impl<S: TryStream> Lines<S> {
    pub fn new(stream: S) -> Self {
        Lines {
            stream: stream.into_stream().fuse(),
            buf: BytesMut::new(),
        }
    }
}

impl<S: TryStream<Error = Error> + Unpin> Stream for Lines<S>
where
    S::Ok: Into<Bytes>,
{
    type Item = Result<Bytes, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(line) = remove_first_line(&mut self.buf) {
            return Poll::Ready(Some(Ok(line.freeze())));
        }

        // Now `self.buf` does not have a CRLF.
        // Extend the buffer until a CRLF is found.

        loop {
            let mut chunk: BytesMut = loop {
                if let Some(c) = ready!(self.stream.poll_next_unpin(cx)) {
                    let c: Bytes = c?.into();
                    if !c.is_empty() {
                        break c.into();
                    }
                } else if !self.buf.is_empty() {
                    let ret = mem::replace(&mut self.buf, BytesMut::new()).freeze();
                    return Poll::Ready(Some(Ok(ret)));
                } else {
                    return Poll::Ready(None);
                }
            };

            if chunk[0] == b'\n' && self.buf.last() == Some(&b'\r') {
                // Drop the CRLF
                chunk.advance(1);
                let line_len = self.buf.len() - 1;
                self.buf.truncate(line_len);
                return Poll::Ready(Some(Ok(mem::replace(&mut self.buf, chunk).freeze())));
            } else if let Some(line) = remove_first_line(&mut chunk) {
                self.buf.unsplit(line);
                return Poll::Ready(Some(Ok(mem::replace(&mut self.buf, chunk).freeze())));
            } else {
                self.buf.unsplit(chunk);
            }
        }
    }
}

impl<F: TryFuture + Unpin> Future for MapErr<F>
where
    F::Error: IntoError,
{
    type Output = Result<F::Ok, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.try_poll_unpin(cx).map_err(IntoError::into_error)
    }
}

impl<S: TryStream + Unpin> Stream for MapErr<S>
where
    S::Error: IntoError,
{
    type Item = Result<S::Ok, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0
            .try_poll_next_unpin(cx)
            .map(|opt| opt.map(|result| result.map_err(IntoError::into_error)))
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

fn remove_first_line(buf: &mut BytesMut) -> Option<BytesMut> {
    if buf.len() < 2 {
        return None;
    }

    if let Some(i) = memchr::memchr(b'\n', &buf[1..]) {
        if buf[i] == b'\r' {
            let mut line = buf.split_to(i + 2);
            line.split_off(i); // Drop the CRLF
            return Some(line);
        }
    }

    None
}

cfg_if! {
    if #[cfg(feature = "runtime")] {
        use std::time::Duration;

        use futures_util::future::{self, Either};
        use futures_util::try_future::IntoFuture;

        pub struct Flatten<S>(IntoStream<S>);

        /// A `tokio_timer::Timeout`-like type that holds the original `Duration`
        /// which can be used to create another `Timeout`.
        pub struct Timeout<F> {
            tokio: tokio_timer::Timeout<IntoFuture<F>>,
            dur: Duration,
        }

        pub type MaybeTimeout<F> = Either<Timeout<F>, MapErr<F>>;

        pub type MaybeTimeoutStream<S> = Either<
            Flatten<tokio_timer::Timeout<IntoStream<S>>>, MapErr<S>,
        >;

        impl<S: TryStream> Flatten<S> {
            pub fn new(s: S) -> Self {
                Flatten(s.into_stream())
            }
        }

        impl<S: TryStream<Ok = Result<T, E>> + Unpin, T, E> Stream for Flatten<S>
        where
            S::Error: IntoError,
            E: IntoError,
        {
            type Item = Result<T, Error>;

            fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                (&mut self.0)
                    .map_err(IntoError::into_error)
                    .and_then(|r| future::ready(r.map_err(IntoError::into_error)))
                    .poll_next_unpin(cx)
            }
        }

        impl<F: TryFuture> Timeout<F> {
            pub fn new(fut: F, dur: Duration) -> Self {
                Timeout {
                    tokio: tokio_timer::Timeout::new(fut.into_future(), dur),
                    dur,
                }
            }
        }

        impl<F: TryFuture + Unpin> Future for Timeout<F>
        where
            F::Error: IntoError,
        {
            type Output = Result<F::Ok, Error>;

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                (&mut self.tokio)
                    .map_err(IntoError::into_error)
                    .and_then(|r| future::ready(r.map_err(IntoError::into_error)))
                    .poll_unpin(cx)
            }
        }

        impl IntoError for tokio_timer::timeout::Elapsed {
            fn into_error(self) -> Error {
                Error::TimedOut
            }
        }

        pub fn timeout<F: TryFuture>(fut: F, dur: Option<Duration>) -> MaybeTimeout<F> {
            match dur {
                Some(dur) => Either::Left(Timeout::new(fut, dur)),
                None => Either::Right(MapErr(fut)),
            }
        }

        pub fn timeout_to_stream<F, S>(timeout: &MaybeTimeout<F>, s: S) -> MaybeTimeoutStream<S>
        where
            S: TryStream,
        {
            match *timeout {
                Either::Left(ref t) => {
                    let timeout = tokio_timer::Timeout::new(s.into_stream(), t.dur);
                    Either::Left(Flatten::new(timeout))
                }
                Either::Right(_) => Either::Right(MapErr(s)),
            }
        }
    } else {
        pub type MaybeTimeout<F> = MapErr<F>;

        pub type MaybeTimeoutStream<S> = MapErr<S>;

        pub fn timeout<F: TryFuture>(fut: F) -> MaybeTimeout<F> {
            MapErr(fut)
        }

        pub fn timeout_to_stream<F, S>(_: &MaybeTimeout<F>, s: S) -> MaybeTimeoutStream<S>
        where
            S: TryStream,
        {
            MapErr(s)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;
    use futures_executor::block_on_stream;
    use futures_util::stream;

    #[test]
    fn lines() {
        let body = [
            "abc\r\n",
            "d\r\nefg\r\n",
            "hi",
            "jk",
            "",
            "\r\n",
            "\r\n",
            "lmn\r\nop",
            "q\rrs\r",
            "\n\n\rtuv\r\r\n",
            "wxyz\n",
        ];

        let concat = body.concat();
        let expected = concat.split("\r\n");
        let lines = Lines::new(stream::iter(&body).map(|&c| Ok(Bytes::from_static(c.as_bytes()))));
        let lines = block_on_stream(lines).map(|s| String::from_utf8(s.unwrap().to_vec()).unwrap());

        assert_eq!(lines.collect::<Vec<_>>(), expected.collect::<Vec<_>>());
    }
}
