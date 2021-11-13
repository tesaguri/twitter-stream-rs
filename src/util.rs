use std::fmt::{self, Display, Formatter};
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes};
use futures_core::{ready, Stream};
use http_body::Body;
use pin_project_lite::pin_project;

use crate::error::Error;

/// Creates an enum with `AsRef<str>` impl.
macro_rules! str_enum {
    (
        $(#[$attr:meta])*
        pub enum $E:ident {
            $(
                $(#[$v_attr:meta])*
                $V:ident = $by:expr
            ),*$(,)?
        }
    ) => {
        $(#[$attr])*
        pub enum $E {
            $(
                $(#[$v_attr])*
                $V,
            )*
        }

        impl std::convert::AsRef<str> for $E {
            fn as_ref(&self) -> &str {
                match *self {
                    $($E::$V => $by,)*
                }
            }
        }
    }
}

pin_project! {
    pub struct Lines<B> {
        #[pin]
        body: B,
        body_done: bool,
        buf: Bytes,
    }
}

impl<B: Body> Lines<B> {
    pub fn new(body: B) -> Self {
        Lines {
            body,
            body_done: false,
            buf: Bytes::new(),
        }
    }

    #[allow(clippy::type_complexity)]
    fn poll_body(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<B::Data, Error<B::Error>>>> {
        let this = self.project();
        if *this.body_done {
            Poll::Ready(None)
        } else if let Some(result) = ready!(this.body.poll_data(cx)) {
            Poll::Ready(Some(result.map_err(Error::Service)))
        } else {
            *this.body_done = true;
            Poll::Ready(None)
        }
    }
}

impl<B: Body> Stream for Lines<B> {
    type Item = Result<Bytes, Error<B::Error>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(line) = remove_first_line(self.as_mut().project().buf) {
            return Poll::Ready(Some(Ok(line)));
        }

        // Now `self.buf` does not have a CRLF.
        // Extend the buffer until a CRLF is found.

        loop {
            let mut chunk = loop {
                if let Some(c) = ready!(self.as_mut().poll_body(cx)?) {
                    if c.has_remaining() {
                        break c;
                    }
                } else if self.buf.is_empty() {
                    return Poll::Ready(None);
                } else {
                    // `self.buf` does not have CRLF so it is safe to return its content as-is.
                    let ret = mem::take(self.as_mut().project().buf);
                    return Poll::Ready(Some(Ok(ret)));
                }
            };

            let this = self.as_mut().project();

            if chunk.chunk()[0] == b'\n' && this.buf.last() == Some(&b'\r') {
                // Drop the CRLF
                this.buf.truncate(this.buf.len() - 1);
                chunk.advance(1);

                let chunk = chunk.copy_to_bytes(chunk.remaining());
                return Poll::Ready(Some(Ok(mem::replace(this.buf, chunk))));
            }

            let mut chunk = chunk.copy_to_bytes(chunk.remaining());

            if let Some(line) = remove_first_line(&mut chunk) {
                let ret = concat_bytes(this.buf, line);
                *this.buf = chunk;
                return Poll::Ready(Some(Ok(ret)));
            }

            *this.buf = concat_bytes(this.buf, chunk);
        }
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

fn remove_first_line(buf: &mut Bytes) -> Option<Bytes> {
    if let Some(i) = memchr::memmem::find(buf, b"\r\n") {
        let mut line = buf.split_to(i + 2);
        line.truncate(i); // Drop the CRLF
        Some(line)
    } else {
        None
    }
}

fn concat_bytes(a: &[u8], b: Bytes) -> Bytes {
    if a.is_empty() {
        b
    } else {
        let mut buf = Vec::with_capacity(a.len() + b.len());
        buf.extend_from_slice(a);
        buf.extend_from_slice(&b);
        buf.into()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;
    use futures::executor::block_on_stream;
    use futures::stream::{self, StreamExt, TryStream};

    pin_project! {
            struct StreamBody<S> {
                #[pin]
                stream: S,
            }
    }

    impl<S: TryStream> Body for StreamBody<S>
    where
        S::Ok: Buf,
    {
        type Data = S::Ok;
        type Error = S::Error;

        fn poll_data(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<Result<S::Ok, S::Error>>> {
            self.project().stream.try_poll_next(cx)
        }

        fn poll_trailers(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
            Poll::Ready(Ok(None))
        }
    }

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
        let lines = Lines::new(StreamBody {
            stream: stream::iter(&body).map(|&c| Ok(Bytes::from_static(c.as_bytes()))),
        });
        let lines = block_on_stream(lines)
            .map(|s: Result<_, Error>| String::from_utf8(s.unwrap().to_vec()).unwrap());

        assert_eq!(lines.collect::<Vec<_>>(), expected.collect::<Vec<_>>());
    }
}
