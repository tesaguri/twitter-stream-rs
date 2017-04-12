use bytes::{BufMut, Bytes};
use chrono::{TimeZone, UTC};
use error::{Error, HyperError};
use futures::{Async, Future, Poll, Stream};
use futures::stream::Fuse;
use serde::de::{Deserialize, Deserializer, Error as SerdeError};
use std::time::Duration;
use tokio_core::reactor::{Handle, Timeout};
use types::DateTime;

macro_rules! string_enums {
    (
        $(
            $(#[$attr:meta])*
            pub enum $E:ident {
                $(
                    $(#[$v_attr:meta])*
                    :$V:ident($by:expr) // The leading (ugly) colon is to suppress local ambiguity error.
                ),*;
                $(#[$u_attr:meta])*
                :$U:ident(_),
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

            impl ::serde::Deserialize for $E {
                fn deserialize<D: ::serde::Deserializer>(d: D) -> ::std::result::Result<Self, D::Error> {
                    struct V;

                    impl ::serde::de::Visitor for V {
                        type Value = $E;

                        fn visit_str<E>(self, s: &str) -> ::std::result::Result<$E, E> {
                            match s {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s.to_owned())),
                            }
                        }

                        fn visit_string<E>(self, s: String) -> ::std::result::Result<$E, E> {
                            match s.as_str() {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s)),
                            }
                        }

                        fn expecting(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                            write!(f, "a string")
                        }
                    }

                    d.deserialize_string(V)
                }
            }

            impl ::std::convert::AsRef<str> for $E {
                fn as_ref(&self) -> &str {
                    match *self {
                        $($E::$V => $by,)*
                        $E::$U(ref s) => s,
                    }
                }
            }

            impl ::std::cmp::PartialEq for $E {
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

            impl ::std::hash::Hash for $E {
                fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                    match *self {
                        $($E::$V => $by.hash(state),)*
                        $E::$U(ref s) => s.hash(state),
                    }
                }
            }

            impl ::std::cmp::Eq for $E {}
        )*
    }
}

pub struct BaseTimeout {
    dur: Duration,
    handle: Handle,
    timer: Timeout,
}

/// Adds a timeout to a `Stream`. Inspired by `tokio_timer::TimeoutStream`.
pub struct TimeoutStream<S> {
    stream: S,
    inner: Option<BaseTimeout>,
}

/// A stream over the lines (delimited by CRLF) of a `Body`.
pub struct Lines<B> where B: Stream {
    inner: Fuse<B>,
    buf: Bytes,
}

impl BaseTimeout {
    pub fn new(dur: Duration, handle: Handle) -> Option<Self> {
        Timeout::new(dur, &handle).ok().map(|timer| {
            BaseTimeout {
                dur: dur,
                handle: handle,
                timer: timer,
            }
        })
    }

    pub fn for_stream<S>(self, stream: S) -> TimeoutStream<S> {
        TimeoutStream {
            stream: stream,
            inner: Timeout::new(self.dur, &self.handle).ok().map(move |timer| {
                BaseTimeout {
                    timer: timer,
                    ..self
                }
            }),
        }
    }

    pub fn timer_mut(&mut self) -> &mut Timeout {
        &mut self.timer
    }
}

impl<S> TimeoutStream<S> {
    pub fn never(stream: S) -> Self {
        TimeoutStream {
            stream: stream,
            inner: None,
        }
    }
}

impl<S> Stream for TimeoutStream<S> where S: Stream<Error=HyperError> {
    type Item = S::Item;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, Error> {
        let ret;

        if let Some(ref mut inner) = self.inner {
            match self.stream.poll() {
                Ok(Async::Ready(Some(v))) => {
                    ret = Ok(Some(v).into());
                    if let Ok(timer) = Timeout::new(inner.dur, &inner.handle) {
                        inner.timer = timer;
                        return ret;
                    }
                    // goto timeout_new_failed;
                },
                Ok(Async::NotReady) => match inner.timer.poll() {
                    Ok(Async::Ready(())) => return Err(Error::TimedOut),
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Err(_) => unreachable!(), // `Timeout` never fails.
                },
                v => return v.map_err(Error::Hyper),
            }
        } else {
            return self.stream.poll().map_err(Error::Hyper);
        }

        // timeout_new_failed:
        self.inner = None;
        ret
    }
}

impl<B> Lines<B> where B: Stream {
    pub fn new(body: B) -> Self {
        Lines {
            inner: body.fuse(),
            buf: Bytes::new(),
        }
    }
}

impl<B> Stream for Lines<B> where B: Stream, B::Item: Into<Bytes> {
    type Item = Bytes;
    type Error = B::Error;

    fn poll(&mut self) -> Poll<Option<Bytes>, B::Error> {
        use std::mem;

        macro_rules! try_ready_some {
            ($poll:expr) => {{
                let poll = $poll;
                match try_ready!(poll) {
                    Some(v) => v,
                    None    => return Ok(None.into()),
                }
            }};
        }

        if self.buf.is_empty() {
            self.buf = try_ready_some!(self.inner.poll()).into();
        }

        fn remove_next_line(buf: &mut Bytes) -> Option<Bytes> {
            (buf as &Bytes).into_iter().enumerate()
                .find(|&(i, b)| b'\r' == b && Some(&b'\n') == buf.get(i + 1))
                .map(|(i, _)| buf.split_off(i + 2))
        }

        if let Some(buf) = remove_next_line(&mut self.buf) {
            let ret = mem::replace(&mut self.buf, buf);
            return Ok(Some(ret).into());
        }

        if self.buf.is_empty() {
            self.buf = try_ready_some!(self.inner.poll()).into();
        }

        // Extend the buffer until a newline is found:
        loop {
            match try_ready!(self.inner.poll()) {
                Some(chunk) => {
                    let mut chunk = chunk.into();
                    if &b'\r' == self.buf.last().unwrap() && Some(&b'\n') == chunk.first() {
                        let i = self.buf.len() + 1;
                        extend_from_bytes(&mut self.buf, chunk);
                        let next = self.buf.split_off(i);
                        let ret = mem::replace(&mut self.buf, next);
                        return Ok(Some(ret).into());
                    } else if let Some(next) = remove_next_line(&mut chunk) {
                        extend_from_bytes(&mut self.buf, chunk);
                        let ret = mem::replace(&mut self.buf, next);
                        return Ok(Some(ret).into());
                    } else {
                        extend_from_bytes(&mut self.buf, chunk);
                    }
                },
                None => {
                    let ret = mem::replace(&mut self.buf, Bytes::new());
                    return Ok(Some(ret).into());
                },
            };
        }
    }
}

pub fn parse_datetime(s: &str) -> ::chrono::format::ParseResult<DateTime> {
    UTC.datetime_from_str(s, "%a %b %e %H:%M:%S %z %Y")
}

pub fn deserialize_datetime<D: Deserializer>(d: D) -> Result<DateTime, D::Error> {
    parse_datetime(&String::deserialize(d)?).map_err(|e| D::Error::custom(e.to_string()))
}

// XXX: possibly can be replaced with carllerche/bytes#85
fn extend_from_bytes(dst: &mut Bytes, src: Bytes) {
    use std::mem;

    if src.is_empty() { return; }

    // Temporary swap just to take the ownership
    let buf = mem::replace(dst, src);

    *dst = match buf.try_mut() {
        Ok(mut buf) => {
            buf.reserve(dst.len());
            buf.put_slice(dst);
            buf.freeze()
        },
        Err(buf) => {
            let mut vec = Vec::with_capacity(buf.len() + dst.len());
            vec.extend_from_slice(&buf);
            vec.extend_from_slice(dst);
            vec.into()
        },
    };
}

#[cfg(test)]
mod test {
    use bytes::Bytes;
    use futures::stream;
    use super::*;

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
            Bytes::from(b"\r\n".to_vec()),
            Bytes::from(b"\r\n".to_vec()),
            Bytes::from(b"lmn\r\nop".to_vec()),
            Bytes::from(b"q\rrs\r".to_vec()),
            Bytes::from(b"\n\n\rtuv\r\r\n".to_vec()),
            Bytes::from(b"wxyz".to_vec()),
        ];

        let mut iter1 = lines.iter().cloned();
        let mut iter2 = Lines::new(stream::iter::<_,_,()>(body.into_iter().map(Ok)))
            .wait().map(|s| String::from_utf8(s.unwrap().to_vec()).unwrap());

        for _ in 0..(lines.len()+1) {
            assert_eq!(
                iter1.next(),
                iter2.next().as_ref().map(String::as_str)
            );
        }
    }
}
