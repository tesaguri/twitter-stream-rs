use bytes::Bytes;
use error::{Error, HyperError};
use futures::{Async, Future, Poll, Stream};
use futures::stream::Fuse;
use std::error;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;
use tokio_core::reactor::{Handle, Timeout};

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

/// Represents an infallible operation in `default_connector::new`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Never {}

impl BaseTimeout {
    pub fn new(dur: Duration, handle: Handle) -> Option<Self> {
        Timeout::new(dur, &handle).ok().map(|timer| {
            BaseTimeout { dur, handle, timer }
        })
    }

    pub fn for_stream<S>(self, stream: S) -> TimeoutStream<S> {
        TimeoutStream {
            stream,
            inner: Timeout::new(self.dur, &self.handle).ok().map(move |timer| {
                BaseTimeout {
                    timer,
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
        TimeoutStream { stream, inner: None }
    }
}

impl<S> Stream for TimeoutStream<S> where S: Stream<Error=HyperError> {
    type Item = S::Item;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, Error> {
        self.stream.poll().map_err(Error::Hyper).and_then(|async| {
            let ret;

            if let Some(ref mut inner) = self.inner {
                match async {
                    Async::Ready(Some(v)) => {
                        ret = Ok(Some(v).into());
                        if let Ok(timer)
                            = Timeout::new(inner.dur, &inner.handle)
                        {
                            inner.timer = timer;
                            return ret;
                        }
                        // goto timeout_new_failed;
                    },
                    Async::Ready(None) => return Ok(Async::Ready(None)),
                    Async::NotReady => match inner.timer.poll() {
                        Ok(Async::Ready(())) => return Err(Error::TimedOut),
                        Ok(Async::NotReady) => return Ok(Async::NotReady),
                        // `Timeout` never fails.
                        Err(_) => unreachable!(),
                    },
                }
            } else {
                return Ok(async);
            }

            // timeout_new_failed:
            self.inner = None;
            ret
        })
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

        fn remove_first_line(buf: &mut Bytes) -> Option<Bytes> {
            (buf as &Bytes).into_iter().enumerate()
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
                    if b'\r' == *self.buf.last().unwrap()
                        && Some(&b'\n') == chunk.first()
                    {
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
                },
                None => {
                    let ret = mem::replace(&mut self.buf, Bytes::new());
                    return Ok(Some(ret).into());
                },
            };
        }
    }
}

impl error::Error for Never {
    fn description(&self) -> &str {
        unreachable!();
    }
}

impl Display for Never {
    fn fmt(&self, _: &mut Formatter) -> fmt::Result {
        unreachable!();
    }
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
            Bytes::from(b"".to_vec()),
            Bytes::from(b"\r\n".to_vec()),
            Bytes::from(b"\r\n".to_vec()),
            Bytes::from(b"lmn\r\nop".to_vec()),
            Bytes::from(b"q\rrs\r".to_vec()),
            Bytes::from(b"\n\n\rtuv\r\r\n".to_vec()),
            Bytes::from(b"wxyz".to_vec()),
        ];

        let mut iter1 = lines.iter().cloned();
        let mut iter2 = Lines::new(stream::iter_ok::<_,()>(body))
            .wait()
            .map(|s| String::from_utf8(s.unwrap().to_vec()).unwrap());

        for _ in 0..(lines.len()+1) {
            assert_eq!(
                iter1.next(),
                iter2.next().as_ref().map(String::as_str)
            );
        }
    }
}
