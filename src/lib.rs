#![feature(proc_macro)]

extern crate chrono;
extern crate futures;
extern crate hyper;
#[macro_use]
extern crate log;
extern crate oauthcli;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json as json;
extern crate url;

pub mod messages;

mod util;

pub use messages::StreamMessage;

use futures::{Async, Future, Poll, Stream};
use hyper::client::Client;
use hyper::status::StatusCode;
use util::{Lines, Timeout};
use std::convert::From;
use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufReader};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Token<'a>(pub &'a str, pub &'a str);

#[derive(Debug, Clone)]
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
    timer: Timeout,
}

#[derive(Debug)]
pub enum Error {
    Url(url::ParseError),
    Hyper(hyper::Error),
    Http(StatusCode),
    Io(io::Error),
    TimedOut(u64),
}

pub type Result<T> = std::result::Result<T, Error>;

impl<'a> TwitterUserStreamBuilder<'a> {
    pub fn new(consumer: Token<'a>, token: Token<'a>) -> Self {
        TwitterUserStreamBuilder {
            consumer: consumer,
            token: token,
            client: None,
            end_point: None,
            timeout: Duration::from_secs(90),
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
            lines: util::lines(BufReader::new(res)),
            timeout: self.timeout,
            timer: Timeout::after(self.timeout),
        })
    }
}

impl TwitterUserStream {
    pub fn login<'a>(consumer: Token<'a>, token: Token<'a>) -> Result<Self> {
        TwitterUserStreamBuilder::new(consumer, token).login()
    }
}

impl Stream for TwitterUserStream {
    type Item = String;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<String>, Error> {
        use Async::*;

        trace!("TwitterUserStream::poll");

        loop {
            match self.lines.poll()? {
                Ready(line_opt) => {
                    match line_opt {
                        Some(line) => {
                            let now = Instant::now();
                            let mut timer = Timeout::after(self.timeout);
                            timer.park(now);
                            info!("duration since last message: {}", {
                                let elapsed = timer.when() - self.timer.when(); // = (now + timeout) - (last + timeout)
                                elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000f64
                            });
                            self.timer = timer;

                            if line.is_empty() {
                                debug!("blank line");
                            } else {
                                return Ok(Ready(Some(line)));
                            }
                        },
                        None => return Ok(None.into()),
                    }
                },
                NotReady => {
                    if let Ok(Ready(())) = self.timer.poll() {
                        return Err(Error::TimedOut(self.timeout.as_secs()));
                    } else {
                        debug!("polled before being ready");
                        return Ok(NotReady);
                    }
                },
            }
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use Error::*;

        match *self {
            Url(ref e) => e.description(),
            Hyper(ref e) => e.description(),
            Http(ref status) => status.canonical_reason().unwrap_or("<unknown status code>"),
            Io(ref e) => e.description(),
            TimedOut(_) => "timed out",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use Error::*;

        match *self {
            Url(ref e) => Some(e),
            Hyper(ref e) => Some(e),
            Http(_) => None,
            Io(ref e) => Some(e),
            TimedOut(_) => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Error::*;

        match *self {
            Url(ref e) => Display::fmt(e, f),
            Hyper(ref e) => Display::fmt(e, f),
            Http(ref code) => Display::fmt(code, f),
            Io(ref e) => Display::fmt(e, f),
            TimedOut(timeout) => write!(f, "connection timed out after {} sec", timeout),
        }
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
