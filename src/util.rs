use std::fmt::{self, Display, Formatter};
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes, BytesMut};
use futures_util::ready;
use futures_util::stream::{Fuse, IntoStream, Stream, StreamExt, TryStream, TryStreamExt};
use http_body::Body;
use pin_project_lite::pin_project;

use crate::error::Error;

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

pin_project! {
    pub struct Lines<S> {
        #[pin]
        stream: Fuse<IntoStream<S>>,
        buf: BytesMut,
    }
}

pin_project! {
    /// Wraps `http_body::Body` to make it a `Stream`.
    pub struct HttpBodyAsStream<B> {
        #[pin]
        pub inner: B,
    }
}

impl<S: TryStream> Lines<S> {
    pub fn new(stream: S) -> Self {
        Lines {
            stream: stream.into_stream().fuse(),
            buf: BytesMut::new(),
        }
    }
}

impl<S: TryStream<Ok = Bytes, Error = Error<E>>, E> Stream for Lines<S> {
    type Item = Result<Bytes, Error<E>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if let Some(line) = remove_first_line(&mut this.buf) {
            return Poll::Ready(Some(Ok(line.freeze())));
        }

        // Now `self.buf` does not have a CRLF.
        // Extend the buffer until a CRLF is found.

        loop {
            let mut chunk: BytesMut = loop {
                if let Some(c) = ready!(this.stream.as_mut().poll_next(cx)) {
                    let c = c?;
                    if !c.is_empty() {
                        // XXX: Copying data to a newly created `BytesMut` because
                        // `impl From<Bytes> for BytesMut` was removed in `bytes` 0.5.
                        break c[..].into();
                    }
                } else if !this.buf.is_empty() {
                    let ret = mem::replace(this.buf, BytesMut::new()).freeze();
                    return Poll::Ready(Some(Ok(ret)));
                } else {
                    return Poll::Ready(None);
                }
            };

            if chunk[0] == b'\n' && this.buf.last() == Some(&b'\r') {
                // Drop the CRLF
                chunk.advance(1);
                let line_len = this.buf.len() - 1;
                this.buf.truncate(line_len);
                return Poll::Ready(Some(Ok(mem::replace(this.buf, chunk).freeze())));
            } else if let Some(line) = remove_first_line(&mut chunk) {
                this.buf.unsplit(line);
                return Poll::Ready(Some(Ok(mem::replace(this.buf, chunk).freeze())));
            } else {
                this.buf.unsplit(chunk);
            }
        }
    }
}

impl<B: Body> HttpBodyAsStream<B> {
    pub fn new(inner: B) -> Self {
        HttpBodyAsStream { inner }
    }
}

impl<B: Body> Stream for HttpBodyAsStream<B> {
    type Item = Result<Bytes, Error<B::Error>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_data(cx).map(|opt| {
            opt.map(|result| result.map(|mut buf| buf.to_bytes()).map_err(Error::Service))
        })
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
            line.truncate(i); // Drop the CRLF
            return Some(line);
        }
    }

    None
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;
    use futures::executor::block_on_stream;
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
        let lines = block_on_stream(lines)
            .map(|s: Result<_, Error>| String::from_utf8(s.unwrap().to_vec()).unwrap());

        assert_eq!(lines.collect::<Vec<_>>(), expected.collect::<Vec<_>>());
    }
}
