extern crate futures;
extern crate hyper;
#[macro_use]
extern crate log;
extern crate oauthcli;
extern crate serde_json;
extern crate url;

mod lines;

use futures::{Async, Poll, Stream};
use futures::task::{self, Task};
use hyper::client::Client;
use hyper::status::StatusCode;
use lines::Lines;
use std::convert::From;
use std::error::Error as StdError;
use std::fmt::{self, Debug, Display, Formatter};
use std::io::{self, BufReader};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Token<'a>(pub &'a str, pub &'a str);

pub struct TwitterUserStreamBuilder<'a> {
    consumer: Token<'a>,
    token: Token<'a>,
    client: Option<&'a Client>,
    end_point: Option<&'a str>,
    timeout: Duration,
    user_agent: Option<&'a str>,
}

pub struct TwitterUserStream {
    lines: Lines,
    timeout: Duration,
    timer: Instant,
    timeout_token: Option<TimeoutToken>,
}

#[derive(Clone)]
struct TimeoutToken(Task, Arc<AtomicBool>);

pub enum Error {
    Url(url::ParseError),
    Hyper(hyper::Error),
    Http(StatusCode),
    Io(io::Error),
    TimedOut,
    InternalPanicError,
}

pub type Result<T> = std::result::Result<T, Error>;

impl<'a> TwitterUserStreamBuilder<'a> {
    pub fn new(consumer: Token<'a>, token: Token<'a>) -> Self {
        TwitterUserStreamBuilder {
            consumer: consumer,
            token: token,
            client: None,
            end_point: None,
            timeout: Duration::from_secs(95),
            user_agent: None,
        }
    }

    pub fn client(&mut self, client: Option<&'a Client>) -> &mut Self {
        self.client = client;
        self
    }

    pub fn end_point(&mut self, url: Option<&'a str>) -> &mut Self {
        self.end_point = url;
        self
    }

    pub fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = timeout;
        self
    }

    pub fn user_agent(&mut self, user_agent: Option<&'a str>) -> &mut Self {
        self.user_agent = user_agent;
        self
    }

    pub fn login(&self) -> Result<TwitterUserStream> {
        use hyper::header::{Headers, Authorization, UserAgent};
        use hyper::status::StatusCode;
        use oauthcli::{OAuthAuthorizationHeaderBuilder, SignatureMethod};
        use url::Url;

        let end_point = self.end_point.unwrap_or("https://userstream.twitter.com/1.1/user.json?with=user");
        let url = Url::parse(end_point)?;

        let auth = OAuthAuthorizationHeaderBuilder::new(
            "GET", &url, self.consumer.0, self.consumer.1, SignatureMethod::HmacSha1)
            .token(self.token.0, self.token.1)
            .finish_for_twitter();

        let mut headers = Headers::new();
        headers.set(Authorization(auth));
        if let Some(ua) = self.user_agent {
            headers.set(UserAgent(ua.to_owned()));
        }

        let res = if let Some(client) = self.client {
            client
                .get(url)
                .headers(headers)
                .send()?
        } else {
            Client::new()
                .get(url)
                .headers(headers)
                .send()?
        };

        match &res.status {
            &StatusCode::Ok => (),
            _ => return Err(res.status.into()),
        }

        Ok(TwitterUserStream {
            lines: lines::lines(BufReader::new(res)),
            timeout: self.timeout,
            timer: Instant::now(),
            timeout_token: None,
        })
    }
}

impl TwitterUserStream {
    pub fn login<'a>(consumer: Token<'a>, token: Token<'a>) -> Result<Self> {
        TwitterUserStreamBuilder::new(consumer, token).login()
    }

    fn update_timeout(&mut self) {
        trace!("TwitterUserStream::update_timeout");

        let task = if let Some(ref t) = self.timeout_token {
            t.1.store(false, Ordering::Relaxed); // Invalidate old token.
            t.0.clone()
        } else {
            task::park()
        };

        self.timeout_token = Some(TimeoutToken(task, Arc::new(AtomicBool::new(true))));

        info!("duration since last update: {}", {
            let elapsed = self.timer.elapsed();
            elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000f64
        });
        self.timer = Instant::now();

        let timeout = self.timeout;
        let token = self.timeout_token.as_ref().unwrap().clone();

        thread::spawn(move || {
            thread::sleep(timeout);
            if token.1.load(Ordering::Relaxed) {
                debug!("unparking");
                token.0.unpark();
            } else {
                debug!("timeout token has expired");
            }
        });
    }
}

impl Stream for TwitterUserStream {
    type Item = String;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<String>, Error> {
        trace!("TwitterUserStream::poll");

        if self.timeout_token.is_none() {
            self.update_timeout();
        }

        match self.lines.poll()? {
            Async::Ready(line_opt) => {
                match line_opt {
                    Some(line) => {
                        self.update_timeout();
                        if !line.is_empty() {
                            return Ok(Some(line).into());
                        } else {
                            return Ok(Async::NotReady);
                        }
                    },
                    None => return Ok(None.into()),
                }
            },
            Async::NotReady => {
                if self.timer.elapsed() >= self.timeout {
                    return Err(Error::TimedOut);
                } else {
                    debug!("polled before being ready");
                    return Ok(Async::NotReady);
                }
            },
        }
    }
}

impl Drop for TimeoutToken {
    fn drop(&mut self) {
        self.1.store(false, Ordering::Relaxed)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            &Error::Url(ref e) => e.description(),
            &Error::Hyper(ref e) => e.description(),
            &Error::Http(ref status) => status.canonical_reason().unwrap_or("unknown HTTP error"),
            &Error::Io(ref e) => e.description(),
            &Error::TimedOut => "the connection has timed out",
            &Error::InternalPanicError => "internal error: sender panicked",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            &Error::Url(ref e) => e.cause(),
            &Error::Hyper(ref e) => e.cause(),
            &Error::Http(_) => None,
            &Error::Io(ref e) => e.cause(),
            &Error::TimedOut => None,
            &Error::InternalPanicError => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Self {
        Error::Url(e)
    }
}

impl From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Self {
        Error::Hyper(e)
    }
}

impl From<StatusCode> for Error {
    fn from(e: StatusCode) -> Self {
        Error::Http(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
